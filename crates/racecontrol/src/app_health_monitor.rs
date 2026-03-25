//! App Health Monitor — probes Next.js app health endpoints every 30 seconds,
//! logs results to SQLite, exposes current status, and fires WhatsApp alerts
//! (with 5-minute per-app cooldown) when any app degrades or becomes unreachable.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::state::AppState;
use crate::whatsapp_alerter;

/// Health status for a single app, returned by the API endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct AppHealthEntry {
    pub app: String,
    pub status: String,
    pub pages_expected: Option<i64>,
    pub pages_available: Option<i64>,
    pub last_checked: String,
    pub response_ms: u64,
    pub error: Option<String>,
}

/// App targets to probe (name, health URL).
const APP_TARGETS: &[(&str, &str)] = &[
    ("admin", "http://192.168.31.23:3201/api/health"),
    ("kiosk", "http://192.168.31.23:3300/kiosk/api/health"),
    ("web", "http://192.168.31.23:3200/api/health"),
];

/// Current health state for all apps (updated every probe cycle).
static CURRENT_HEALTH: LazyLock<RwLock<Vec<AppHealthEntry>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));

/// Per-app last alert time for 5-minute cooldown.
static ALERT_COOLDOWN: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Per-app previous status for transition detection (degraded/unreachable -> ok = recovery).
static PREV_STATUS: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 5-minute cooldown per app (300 seconds).
const ALERT_COOLDOWN_SECS: u64 = 300;

/// Spawn the app health monitor background task.
pub fn spawn(state: Arc<AppState>) {
    tracing::info!(target: "app_health_monitor", "App health monitor starting (30s interval)");

    tokio::spawn(async move {
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(target: "app_health_monitor", "Failed to build HTTP client: {}", e);
                return;
            }
        };

        let mut interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            interval.tick().await;

            // Probe all 3 apps concurrently
            let (admin, kiosk, web) = tokio::join!(
                probe_app(&client, APP_TARGETS[0].0, APP_TARGETS[0].1),
                probe_app(&client, APP_TARGETS[1].0, APP_TARGETS[1].1),
                probe_app(&client, APP_TARGETS[2].0, APP_TARGETS[2].1),
            );

            let entries = vec![admin, kiosk, web];

            // Update static health state
            if let Ok(mut health) = CURRENT_HEALTH.write() {
                *health = entries.clone();
            }

            // WhatsApp alerting (only if enabled)
            if state.config.alerting.enabled {
                for entry in &entries {
                    handle_alert(&state, entry).await;
                }
            }

            // Fire-and-forget DB logging
            let db = state.db.clone();
            let log_entries = entries.clone();
            tokio::spawn(async move {
                for entry in &log_entries {
                    log_health_to_db(&db, entry).await;
                }
            });
        }
    });
}

/// Probe a single app's health endpoint.
async fn probe_app(client: &reqwest::Client, name: &str, url: &str) -> AppHealthEntry {
    let start = Instant::now();
    let now_str = whatsapp_alerter::ist_now_string();

    match client.get(url).send().await {
        Ok(resp) => {
            let response_ms = start.elapsed().as_millis() as u64;
            let http_status = resp.status();

            match resp.text().await {
                Ok(body) => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        let status = if http_status.is_success() {
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

                        AppHealthEntry {
                            app: name.to_string(),
                            status,
                            pages_expected,
                            pages_available,
                            last_checked: now_str,
                            response_ms,
                            error: None,
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
                        }
                    }
                }
                Err(e) => AppHealthEntry {
                    app: name.to_string(),
                    status: "degraded".to_string(),
                    pages_expected: None,
                    pages_available: None,
                    last_checked: now_str,
                    response_ms,
                    error: Some(format!("Failed to read response body: {}", e)),
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
            }
        }
    }
}

/// Handle WhatsApp alerting for a single app entry (cooldown + transition detection).
async fn handle_alert(state: &AppState, entry: &AppHealthEntry) {
    let prev = {
        let map = PREV_STATUS.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&entry.app).cloned().unwrap_or_else(|| "ok".to_string())
    };

    let is_bad = entry.status == "degraded" || entry.status == "unreachable";
    let was_bad = prev == "degraded" || prev == "unreachable";

    if is_bad {
        // Check cooldown
        let can_alert = {
            let map = ALERT_COOLDOWN.lock().unwrap_or_else(|e| e.into_inner());
            match map.get(&entry.app) {
                Some(last) => last.elapsed().as_secs() >= ALERT_COOLDOWN_SECS,
                None => true,
            }
        };

        if can_alert {
            let detail = if entry.status == "unreachable" {
                "endpoint not responding".to_string()
            } else {
                match (entry.pages_available, entry.pages_expected) {
                    (Some(avail), Some(expected)) => format!("{}/{} pages available", avail, expected),
                    _ => "degraded response".to_string(),
                }
            };

            let msg = format!(
                "[APP HEALTH] {} {}: {}. {}",
                entry.app, entry.status, detail, whatsapp_alerter::ist_now_string()
            );

            whatsapp_alerter::send_whatsapp(&state.config, &msg).await;

            // Update cooldown
            if let Ok(mut map) = ALERT_COOLDOWN.lock() {
                map.insert(entry.app.clone(), Instant::now());
            }
        }
    } else if !is_bad && was_bad {
        // Recovery notification (no cooldown on recovery)
        let msg = format!(
            "[APP HEALTH] {} recovered: all pages healthy. {}",
            entry.app, whatsapp_alerter::ist_now_string()
        );
        whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
    }

    // Update previous status
    if let Ok(mut map) = PREV_STATUS.lock() {
        map.insert(entry.app.clone(), entry.status.clone());
    }
}

/// Log a health entry to the app_health_log table (best-effort).
async fn log_health_to_db(db: &sqlx::SqlitePool, entry: &AppHealthEntry) {
    let id = uuid::Uuid::new_v4().to_string();
    let result = sqlx::query(
        "INSERT INTO app_health_log (id, app, timestamp, status, pages_expected, pages_available, response_ms, error) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&entry.app)
    .bind(&entry.last_checked)
    .bind(&entry.status)
    .bind(entry.pages_expected)
    .bind(entry.pages_available)
    .bind(entry.response_ms as i64)
    .bind(&entry.error)
    .execute(db)
    .await;

    if let Err(e) = result {
        tracing::warn!(target: "app_health_monitor", "Failed to log health entry for {}: {}", entry.app, e);
    }
}

/// Get the current health status of all monitored apps.
pub async fn get_current_health() -> Vec<AppHealthEntry> {
    match CURRENT_HEALTH.read() {
        Ok(health) => health.clone(),
        Err(e) => {
            tracing::warn!(target: "app_health_monitor", "Failed to read health state: {}", e);
            Vec::new()
        }
    }
}
