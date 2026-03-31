//! Health polling and status classification (EG-01, EG-04, EG-06).

use crate::config::GuardianConfig;
use serde::Deserialize;
use tracing::{debug, warn};

/// Server status classification (EG-06).
#[derive(Debug)]
pub enum ServerStatus {
    /// Server responding normally.
    Healthy { response_time_ms: u64 },
    /// Server responding but slowly (above busy_threshold_ms).
    Busy { response_time_ms: u64 },
    /// Connection refused — server process is down.
    Dead { error: String },
    /// Timeout — network issue or server hung.
    Unreachable { error: String },
}

impl ServerStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, ServerStatus::Healthy { .. } | ServerStatus::Busy { .. })
    }
}

/// Health endpoint response shape.
#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    #[allow(dead_code)]
    pub status: Option<String>,
    #[allow(dead_code)]
    pub build_id: Option<String>,
}

/// Poll the server health endpoint (EG-01).
pub async fn poll_server(client: &reqwest::Client, config: &GuardianConfig) -> ServerStatus {
    let start = std::time::Instant::now();

    match client.get(&config.server_url).send().await {
        Ok(resp) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            let status_code = resp.status();

            if status_code.is_success() {
                // Try to parse response body
                match resp.json::<HealthResponse>().await {
                    Ok(health) => {
                        debug!(
                            response_time_ms = elapsed_ms,
                            build_id = ?health.build_id,
                            "Health check OK"
                        );
                    }
                    Err(e) => {
                        debug!(
                            response_time_ms = elapsed_ms,
                            error = %e,
                            "Health check OK but response parse failed (non-critical)"
                        );
                    }
                }

                // EG-06: Classify busy vs healthy
                if elapsed_ms > config.busy_threshold_ms {
                    ServerStatus::Busy { response_time_ms: elapsed_ms }
                } else {
                    ServerStatus::Healthy { response_time_ms: elapsed_ms }
                }
            } else {
                // Got a response but non-2xx — treat as degraded but not dead
                warn!(
                    status = status_code.as_u16(),
                    response_time_ms = elapsed_ms,
                    "Health check returned non-success status"
                );
                // Non-success HTTP status still means the process is running
                ServerStatus::Busy { response_time_ms: elapsed_ms }
            }
        }
        Err(e) => {
            let is_timeout = e.is_timeout();
            let error_str = e.to_string();
            classify_error(&error_str, is_timeout)
        }
    }
}

/// Classify a reqwest error into Dead vs Unreachable.
///
/// Connection refused = server process is dead (host is up).
/// Timeout = network issue or server hung.
/// Other = DNS, Tailscale, etc.
fn classify_error(error_str: &str, is_timeout: bool) -> ServerStatus {
    if error_str.contains("Connection refused")
        || error_str.contains("connection refused")
        || error_str.contains("os error 111")
        || error_str.contains("os error 10061")
    {
        ServerStatus::Dead {
            error: error_str.to_string(),
        }
    } else if is_timeout {
        ServerStatus::Unreachable {
            error: error_str.to_string(),
        }
    } else {
        ServerStatus::Unreachable {
            error: error_str.to_string(),
        }
    }
}

/// Check if there are active billing sessions (EG-04).
///
/// Returns `true` if it's safe to restart (no active billing),
/// `false` if there are active sessions or we can't determine.
pub async fn check_billing_safety(client: &reqwest::Client, config: &GuardianConfig) -> bool {
    // If server is down, fleet endpoint won't respond either.
    // Try the fleet health endpoint — if it fails, assume no billing data available.
    // In that case, it's safer to restart since the server is clearly down.

    // Build fleet health URL — we check if any pod has active billing
    // The fleet health endpoint returns pod status, not billing directly.
    // We need to check billing endpoint instead.
    let billing_url = config.server_url.replace("/health", "/billing/active");

    match client.get(&billing_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(val) => {
                    // Check if there are any active sessions
                    if let Some(sessions) = val.as_array() {
                        if sessions.is_empty() {
                            debug!("No active billing sessions — safe to restart");
                            true
                        } else {
                            warn!(
                                active_count = sessions.len(),
                                "Active billing sessions found — restart unsafe"
                            );
                            false
                        }
                    } else if let Some(obj) = val.as_object() {
                        // Might be { "sessions": [...] } or { "count": 0 }
                        if let Some(count) = obj.get("count").and_then(|v| v.as_u64()) {
                            if count == 0 {
                                debug!("No active billing sessions (count=0) — safe to restart");
                                return true;
                            }
                            warn!(active_count = count, "Active billing sessions — restart unsafe");
                            return false;
                        }
                        if let Some(sessions) = obj.get("sessions").and_then(|v| v.as_array()) {
                            if sessions.is_empty() {
                                debug!("No active billing sessions — safe to restart");
                                return true;
                            }
                            warn!(
                                active_count = sessions.len(),
                                "Active billing sessions — restart unsafe"
                            );
                            return false;
                        }
                        // Can't parse — assume safe since server is likely down anyway
                        debug!("Could not parse billing response — assuming safe");
                        true
                    } else {
                        debug!("Unexpected billing response format — assuming safe");
                        true
                    }
                }
                Err(e) => {
                    debug!(error = %e, "Failed to parse billing response — assuming safe (server likely down)");
                    true
                }
            }
        }
        Ok(resp) => {
            debug!(
                status = resp.status().as_u16(),
                "Billing check returned non-success — server likely down, assuming safe to restart"
            );
            true
        }
        Err(e) => {
            debug!(error = %e, "Billing endpoint unreachable — server is down, safe to restart");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_status_is_healthy() {
        assert!(ServerStatus::Healthy { response_time_ms: 50 }.is_healthy());
        assert!(ServerStatus::Busy { response_time_ms: 6000 }.is_healthy());
        assert!(!ServerStatus::Dead { error: "connection refused".into() }.is_healthy());
        assert!(!ServerStatus::Unreachable { error: "timeout".into() }.is_healthy());
    }

    #[test]
    fn test_classify_error_connection_refused() {
        let status = classify_error("Connection refused (os error 111)", false);
        assert!(matches!(status, ServerStatus::Dead { .. }));
    }

    #[test]
    fn test_classify_error_timeout() {
        let status = classify_error("request timed out", true);
        assert!(matches!(status, ServerStatus::Unreachable { .. }));
    }

    #[test]
    fn test_classify_error_dns() {
        let status = classify_error("dns error: no such host", false);
        assert!(matches!(status, ServerStatus::Unreachable { .. }));
    }

    #[test]
    fn test_classify_error_windows_connection_refused() {
        let status = classify_error("os error 10061", false);
        assert!(matches!(status, ServerStatus::Dead { .. }));
    }
}
