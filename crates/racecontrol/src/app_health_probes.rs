//! Health probing logic for the app health monitor.
//! Extracted from app_health_monitor.rs for ARCH-03 (<500 line modules).

use std::time::{Duration, Instant};

use super::{AppHealthEntry, RESPONSE_TIME_SLA_MS};
use crate::whatsapp_alerter;

/// Probe a single app's health endpoint with semantic validation.
/// If `deep_url` is provided, also runs the deep health probe.
pub(crate) async fn probe_app(
    client: &reqwest::Client,
    name: &str,
    url: &str,
    deep_url: Option<&str>,
) -> AppHealthEntry {
    let start = Instant::now();
    let now_str = whatsapp_alerter::ist_now_string();

    // Retry-once before declaring unreachable (standing rule: never conclude offline from single probe)
    let http_result = match client.get(url).send().await {
        Ok(resp) => Ok(resp),
        Err(_first_err) => {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            client.get(url).send().await
        }
    };

    let mut entry = match http_result {
        Ok(resp) => {
            let response_ms = start.elapsed().as_millis() as u64;
            let http_status = resp.status();

            match resp.text().await {
                Ok(body) => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        let mut status = if http_status.is_success() {
                            json.get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("ok")
                                .to_string()
                        } else {
                            "degraded".to_string()
                        };

                        let pages_expected = json
                            .pointer("/deploy/pages_expected")
                            .and_then(|v| v.as_i64());
                        let pages_available = json
                            .pointer("/deploy/pages_available")
                            .and_then(|v| v.as_i64());

                        let mut error = None;

                        // Content assertion: pages_available < pages_expected = degraded
                        if let (Some(avail), Some(expected)) = (pages_available, pages_expected)
                            && avail < expected && status == "ok" {
                                status = "degraded".to_string();
                                error = Some(format!(
                                    "Missing pages: {}/{} available",
                                    avail, expected
                                ));
                            }

                        // Response time SLA: slow response
                        if response_ms > RESPONSE_TIME_SLA_MS && status == "ok" {
                            status = "slow".to_string();
                            error = Some(format!(
                                "Response time {}ms exceeds {}ms SLA",
                                response_ms, RESPONSE_TIME_SLA_MS
                            ));
                        }

                        AppHealthEntry {
                            app: name.to_string(),
                            status,
                            pages_expected,
                            pages_available,
                            last_checked: now_str,
                            response_ms,
                            error,
                            semantic_status: None,
                            deep_check_passed: None,
                        }
                    } else {
                        AppHealthEntry {
                            app: name.to_string(),
                            status: "degraded".to_string(),
                            pages_expected: None,
                            pages_available: None,
                            last_checked: now_str,
                            response_ms,
                            error: Some("Invalid JSON response".to_string()),
                            semantic_status: None,
                            deep_check_passed: None,
                        }
                    }
                }
                Err(e) => AppHealthEntry {
                    app: name.to_string(),
                    status: "degraded".to_string(),
                    pages_expected: None,
                    pages_available: None,
                    last_checked: now_str,
                    response_ms: start.elapsed().as_millis() as u64,
                    error: Some(format!("Failed to read response body: {}", e)),
                    semantic_status: None,
                    deep_check_passed: None,
                },
            }
        }
        Err(e) => {
            let response_ms = start.elapsed().as_millis() as u64;
            AppHealthEntry {
                app: name.to_string(),
                status: "unreachable".to_string(),
                pages_expected: None,
                pages_available: None,
                last_checked: now_str,
                response_ms,
                error: Some(format!("Endpoint not responding: {}", e)),
                semantic_status: None,
                deep_check_passed: None,
            }
        }
    };

    // Deep health probe (only if URL provided and basic health is ok/slow)
    if let Some(deep) = deep_url
        && (entry.status == "ok" || entry.status == "slow") {
            match probe_deep(client, deep).await {
                Ok((passed, semantic)) => {
                    entry.deep_check_passed = Some(passed);
                    entry.semantic_status = Some(semantic.clone());
                    if !passed && entry.status == "ok" {
                        entry.status = "degraded".to_string();
                        entry.error = Some(format!("Deep health check failed: {}", semantic));
                    }
                }
                Err(e) => {
                    entry.deep_check_passed = Some(false);
                    entry.semantic_status = Some(format!("probe_error: {}", e));
                    // Don't downgrade status for deep probe errors — it's supplementary
                    tracing::warn!(
                        target: "app_health_monitor",
                        "Deep probe failed for {}: {}", name, e
                    );
                }
            }
        }

    entry
}

/// Run a deep health probe against a `/api/health/deep` endpoint.
/// Returns (passed: bool, summary: String).
async fn probe_deep(
    client: &reqwest::Client,
    url: &str,
) -> Result<(bool, String), String> {
    let resp = client
        .get(url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("unreachable: {}", e))?;

    let body = resp
        .text()
        .await
        .map_err(|e| format!("body read error: {}", e))?;

    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("invalid JSON: {}", e))?;

    let passed = json
        .get("healthy")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let summary = json
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("no summary")
        .to_string();

    Ok((passed, summary))
}
