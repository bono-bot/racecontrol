//! Synthetic Transaction Monitor — periodically runs golden-path API checks
//! to catch silent failures where health=OK but actual flows are broken.
//!
//! Checks run every 5 minutes:
//! 1. Fleet health returns >0 pods
//! 2. Games catalog is non-empty
//! 3. Config API returns valid JSON
//! 4. Billing pricing tiers exist
//!
//! After 2 consecutive failures of any check → WhatsApp alert.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use serde::Serialize;

use crate::state::AppState;
use crate::whatsapp_alerter;

/// Interval between synthetic probe runs (5 minutes).
const PROBE_INTERVAL_SECS: u64 = 300;

/// Consecutive failures before alerting.
const ALERT_THRESHOLD: u32 = 2;

/// Track consecutive failure counts per probe.
static FAILURE_COUNTS: LazyLock<Mutex<HashMap<String, u32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Result of a single synthetic probe.
#[derive(Debug, Clone, Serialize)]
pub struct SyntheticResult {
    pub probe_name: String,
    pub passed: bool,
    pub response_ms: u64,
    pub error: Option<String>,
    pub timestamp: String,
}

/// Golden-path probes to run (name, URL, validation description).
const PROBES: &[(&str, &str)] = &[
    ("fleet_health", "http://127.0.0.1:8080/api/v1/fleet/health"),
    ("games_catalog", "http://127.0.0.1:8080/api/v1/games"),
    ("config_api", "http://127.0.0.1:8080/api/v1/config"),
    ("billing_pricing", "http://127.0.0.1:8080/api/v1/pricing"),
];

/// Spawn the synthetic transaction monitor background task.
pub fn spawn(state: Arc<AppState>) {
    tracing::info!(
        target: "synthetic_monitor",
        "Synthetic monitor starting ({}s interval, {} probes)",
        PROBE_INTERVAL_SECS,
        PROBES.len()
    );

    tokio::spawn(async move {
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(target: "synthetic_monitor", "Failed to build HTTP client: {}", e);
                return;
            }
        };

        // Initial delay — let server fully boot before probing itself
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(PROBE_INTERVAL_SECS));

        loop {
            interval.tick().await;

            let mut results = Vec::new();

            for (name, url) in PROBES {
                let result = run_probe(&client, name, url).await;
                results.push(result);
            }

            // Track failures and alert
            for result in &results {
                let count = {
                    let mut map = FAILURE_COUNTS.lock().unwrap_or_else(|e| e.into_inner());
                    if result.passed {
                        map.insert(result.probe_name.clone(), 0);
                        0
                    } else {
                        let count = map.entry(result.probe_name.clone()).or_insert(0);
                        *count += 1;
                        *count
                    }
                };

                if !result.passed && count == ALERT_THRESHOLD && state.config.alerting.enabled {
                    let msg = format!(
                        "[SYNTHETIC] {} FAILED {}x: {}. {}",
                        result.probe_name,
                        count,
                        result.error.as_deref().unwrap_or("unknown"),
                        whatsapp_alerter::ist_now_string()
                    );
                    whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                }
            }

            // Fire-and-forget DB logging
            let db = state.db.clone();
            let log_results = results.clone();
            tokio::spawn(async move {
                for result in &log_results {
                    log_synthetic_to_db(&db, result).await;
                }
            });
        }
    });
}

/// Run a single golden-path probe.
async fn run_probe(client: &reqwest::Client, name: &str, url: &str) -> SyntheticResult {
    let start = std::time::Instant::now();
    let now_str = whatsapp_alerter::ist_now_string();

    match client.get(url).send().await {
        Ok(resp) => {
            let response_ms = start.elapsed().as_millis() as u64;
            if !resp.status().is_success() {
                return SyntheticResult {
                    probe_name: name.to_string(),
                    passed: false,
                    response_ms,
                    error: Some(format!("HTTP {}", resp.status())),
                    timestamp: now_str,
                };
            }

            match resp.text().await {
                Ok(body) => {
                    let (passed, error) = validate_response(name, &body);
                    SyntheticResult {
                        probe_name: name.to_string(),
                        passed,
                        response_ms,
                        error,
                        timestamp: now_str,
                    }
                }
                Err(e) => SyntheticResult {
                    probe_name: name.to_string(),
                    passed: false,
                    response_ms,
                    error: Some(format!("body read error: {}", e)),
                    timestamp: now_str,
                },
            }
        }
        Err(e) => SyntheticResult {
            probe_name: name.to_string(),
            passed: false,
            response_ms: start.elapsed().as_millis() as u64,
            error: Some(format!("unreachable: {}", e)),
            timestamp: now_str,
        },
    }
}

/// Validate a probe response based on the probe name.
fn validate_response(probe_name: &str, body: &str) -> (bool, Option<String>) {
    let json: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return (false, Some(format!("invalid JSON: {}", e))),
    };

    match probe_name {
        "fleet_health" => {
            // Expect an array with >0 items, or an object with "pods" array
            let pods = if json.is_array() {
                json.as_array()
            } else {
                json.get("pods").and_then(|v| v.as_array())
            };
            match pods {
                Some(arr) if !arr.is_empty() => (true, None),
                Some(_) => (false, Some("fleet health returned 0 pods".to_string())),
                None => (false, Some("no pods array in response".to_string())),
            }
        }
        "games_catalog" => {
            let games = json.get("games").and_then(|v| v.as_array());
            match games {
                Some(arr) if !arr.is_empty() => (true, None),
                Some(_) => (false, Some("games catalog empty".to_string())),
                None => (false, Some("no games array in response".to_string())),
            }
        }
        "config_api" => {
            if json.is_object() && !json.as_object().map_or(true, |m| m.is_empty()) {
                (true, None)
            } else {
                (false, Some("config returned empty object".to_string()))
            }
        }
        "billing_pricing" => {
            let tiers = json.get("tiers").and_then(|v| v.as_array());
            match tiers {
                Some(arr) if !arr.is_empty() => (true, None),
                Some(_) => (false, Some("no pricing tiers".to_string())),
                None => (false, Some("no tiers array in response".to_string())),
            }
        }
        _ => (true, None),
    }
}

/// Log a synthetic probe result to the database (best-effort).
async fn log_synthetic_to_db(db: &sqlx::SqlitePool, result: &SyntheticResult) {
    let id = uuid::Uuid::new_v4().to_string();
    let r = sqlx::query(
        "INSERT INTO synthetic_probes (id, probe_name, passed, response_ms, error, timestamp) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&result.probe_name)
    .bind(result.passed)
    .bind(result.response_ms as i64)
    .bind(&result.error)
    .bind(&result.timestamp)
    .execute(db)
    .await;

    if let Err(e) = r {
        tracing::warn!(
            target: "synthetic_monitor",
            "Failed to log synthetic result for {}: {}",
            result.probe_name, e
        );
    }
}
