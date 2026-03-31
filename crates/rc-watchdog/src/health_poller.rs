//! SW-05: Startup health polling — 3 attempts x 10s intervals.
//!
//! After rc-agent is restarted (by the watchdog or after a rollback), this module
//! polls rc-agent's /health endpoint to verify it actually came up and is serving.
//!
//! Uses blocking reqwest (no tokio) since the service loop is already synchronous.

use std::time::{Duration, Instant};

/// Default rc-agent health endpoint.
const DEFAULT_AGENT_HEALTH_URL: &str = "http://127.0.0.1:8090/health";

/// Number of health check attempts before declaring failure.
const MAX_HEALTH_ATTEMPTS: u32 = 3;

/// Delay between health check attempts.
const HEALTH_POLL_INTERVAL: Duration = Duration::from_secs(10);

/// HTTP timeout for each health check request.
const HEALTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Result of a health poll sequence.
#[derive(Debug, Clone)]
pub struct HealthPollResult {
    pub healthy: bool,
    pub attempts: u32,
    pub total_duration: Duration,
    pub last_error: Option<String>,
    /// The build_id reported by the agent (if we got a valid response)
    pub build_id: Option<String>,
}

/// Agent health response structure.
#[derive(Debug, serde::Deserialize)]
struct AgentHealthResponse {
    #[serde(default)]
    build_id: String,
    #[serde(default)]
    pod_id: String,
    #[serde(default)]
    uptime_secs: u64,
}

/// Poll rc-agent /health endpoint up to MAX_HEALTH_ATTEMPTS times.
///
/// Returns `HealthPollResult` indicating whether the agent came up healthy.
/// Never panics — all errors are captured in the result.
pub fn poll_agent_health() -> HealthPollResult {
    poll_agent_health_at(DEFAULT_AGENT_HEALTH_URL)
}

/// Poll a specific health URL — useful for testing.
pub fn poll_agent_health_at(url: &str) -> HealthPollResult {
    let client = match reqwest::blocking::Client::builder()
        .timeout(HEALTH_REQUEST_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to build HTTP client for health poll: {}", e);
            return HealthPollResult {
                healthy: false,
                attempts: 0,
                total_duration: Duration::ZERO,
                last_error: Some(format!("HTTP client build failed: {}", e)),
                build_id: None,
            };
        }
    };

    let start = Instant::now();
    let mut last_error = None;

    for attempt in 1..=MAX_HEALTH_ATTEMPTS {
        tracing::info!(
            "Health poll attempt {}/{} for {}",
            attempt, MAX_HEALTH_ATTEMPTS, url
        );

        match client.get(url).send() {
            Ok(resp) if resp.status().is_success() => {
                // Try to parse the response body for build_id
                let build_id = resp.json::<AgentHealthResponse>()
                    .ok()
                    .map(|r| r.build_id)
                    .filter(|id| !id.is_empty());

                tracing::info!(
                    "Health poll succeeded on attempt {}/{} (build_id: {:?})",
                    attempt, MAX_HEALTH_ATTEMPTS, build_id
                );
                return HealthPollResult {
                    healthy: true,
                    attempts: attempt,
                    total_duration: start.elapsed(),
                    last_error: None,
                    build_id,
                };
            }
            Ok(resp) => {
                let err_msg = format!("HTTP {}", resp.status());
                tracing::warn!(
                    "Health poll attempt {}/{} failed: {}",
                    attempt, MAX_HEALTH_ATTEMPTS, err_msg
                );
                last_error = Some(err_msg);
            }
            Err(e) => {
                let err_msg = format!("{}", e);
                tracing::warn!(
                    "Health poll attempt {}/{} failed: {}",
                    attempt, MAX_HEALTH_ATTEMPTS, err_msg
                );
                last_error = Some(err_msg);
            }
        }

        // Wait before next attempt (but not after the last attempt)
        if attempt < MAX_HEALTH_ATTEMPTS {
            std::thread::sleep(HEALTH_POLL_INTERVAL);
        }
    }

    tracing::error!(
        "Health poll failed after {} attempts ({:?})",
        MAX_HEALTH_ATTEMPTS,
        start.elapsed()
    );

    HealthPollResult {
        healthy: false,
        attempts: MAX_HEALTH_ATTEMPTS,
        total_duration: start.elapsed(),
        last_error,
        build_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_poll_unreachable_host() {
        // Poll a definitely-not-listening address — should fail gracefully
        let result = poll_agent_health_at("http://127.0.0.1:19999/health");
        assert!(!result.healthy);
        assert_eq!(result.attempts, MAX_HEALTH_ATTEMPTS);
        assert!(result.last_error.is_some());
        assert!(result.build_id.is_none());
    }

    #[test]
    fn test_health_poll_result_defaults() {
        let result = HealthPollResult {
            healthy: false,
            attempts: 0,
            total_duration: Duration::ZERO,
            last_error: None,
            build_id: None,
        };
        assert!(!result.healthy);
        assert_eq!(result.attempts, 0);
    }
}
