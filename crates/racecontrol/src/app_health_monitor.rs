//! App Health Monitor — probes Next.js app health endpoints every 30 seconds,
//! logs results to SQLite, exposes current status, and fires WhatsApp alerts
//! (with 5-minute per-app cooldown) when any app degrades or becomes unreachable.
//!
//! v38.0: Added semantic health validation — content assertions, response time SLA,
//! deep health probes, dependency-chain awareness, and server app auto-restart via pm2.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::state::AppState;
use crate::whatsapp_alerter;

/// Response time SLA threshold (ms). Above this -> "slow" status.
const RESPONSE_TIME_SLA_MS: u64 = 3000;

/// Deep health probe interval: every 5th cycle (150s = 5 * 30s).
const DEEP_PROBE_EVERY_N_CYCLES: u64 = 5;

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
    /// Semantic status from deep health probe.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_status: Option<String>,
    /// Whether the deep health probe passed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deep_check_passed: Option<bool>,
}

/// App targets to probe (name, health URL, deep health URL).
const APP_TARGETS: &[(&str, &str, Option<&str>)] = &[
    ("admin", "http://192.168.31.23:3201/api/health", None),
    (
        "kiosk",
        "http://192.168.31.23:3300/kiosk/api/health",
        Some("http://192.168.31.23:3300/kiosk/api/health/deep"),
    ),
    (
        "web",
        "http://192.168.31.23:3200/api/health",
        Some("http://192.168.31.23:3200/api/health/deep"),
    ),
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

/// Consecutive failure counter per app for restart triggers (Phase 3).
static CONSECUTIVE_FAILURES: LazyLock<Mutex<HashMap<String, u32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Spawn the app health monitor background task.
pub fn spawn(state: Arc<AppState>) {
    tracing::info!(target: "app_health_monitor", "App health monitor starting (30s interval, deep probe every {}th cycle)", DEEP_PROBE_EVERY_N_CYCLES);

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
        let mut cycle_count: u64 = 0;

        loop {
            interval.tick().await;
            cycle_count += 1;

            let run_deep = cycle_count % DEEP_PROBE_EVERY_N_CYCLES == 0;

            // Probe all 3 apps concurrently
            let (admin, kiosk, web) = tokio::join!(
                probe_app(&client, APP_TARGETS[0].0, APP_TARGETS[0].1, if run_deep { APP_TARGETS[0].2 } else { None }),
                probe_app(&client, APP_TARGETS[1].0, APP_TARGETS[1].1, if run_deep { APP_TARGETS[1].2 } else { None }),
                probe_app(&client, APP_TARGETS[2].0, APP_TARGETS[2].1, if run_deep { APP_TARGETS[2].2 } else { None }),
            );

            let entries = vec![admin, kiosk, web];

            // Update static health state
            if let Ok(mut health) = CURRENT_HEALTH.write() {
                *health = entries.clone();
            }

            // Track consecutive failures per app
            for entry in &entries {
                let is_bad = entry.status == "degraded"
                    || entry.status == "unreachable"
                    || entry.status == "slow";
                if let Ok(mut map) = CONSECUTIVE_FAILURES.lock() {
                    if is_bad {
                        let count = map.entry(entry.app.clone()).or_insert(0);
                        *count += 1;
                    } else {
                        map.insert(entry.app.clone(), 0);
                    }
                }
            }

            // Dependency-chain-aware alerting (Phase 2)
            if state.config.alerting.enabled {
                crate::dependency_chain::evaluate_and_alert(&state, &entries).await;
            }

            // MI Bridge: Log persistent app degradation as fleet incidents.
            // After 5 consecutive failures (~2.5 min at 30s interval), record an
            // incident in the fleet KB so Meshed Intelligence can learn from it.
            // This bridges the gap where app_health_monitor detected 11,535 kiosk
            // errors but MI never knew about them (no DiagnosticEvent was emitted).
            {
                let consecutive = CONSECUTIVE_FAILURES.lock().ok();
                for entry in &entries {
                    let count = consecutive.as_ref()
                        .and_then(|m| m.get(&entry.app).copied())
                        .unwrap_or(0);
                    // Emit incident at 5 consecutive failures, then every 20th after that
                    if count == 5 || (count > 5 && count % 20 == 0) {
                        let incident = rc_common::mesh_types::MeshIncident {
                            id: format!("inc_app_{}_{}", entry.app, chrono::Utc::now().timestamp()),
                            node: "server".to_string(),
                            problem_key: format!("app_degraded:{}", entry.app),
                            severity: if count >= 20 {
                                rc_common::mesh_types::IncidentSeverity::High
                            } else {
                                rc_common::mesh_types::IncidentSeverity::Medium
                            },
                            cost: 0.0,
                            resolution: None,
                            time_to_resolve_secs: None,
                            resolved_by_tier: None,
                            detected_at: chrono::Utc::now(),
                            resolved_at: None,
                        };
                        let db = state.db.clone();
                        let app_name = entry.app.clone();
                        let error_msg = entry.error.clone().unwrap_or_default();
                        let fail_count = count;
                        tokio::spawn(async move {
                            if let Err(e) = crate::fleet_kb::insert_incident(&db, &incident).await {
                                tracing::warn!(
                                    target: "app_health_monitor",
                                    "Failed to log MI incident for {}: {}", app_name, e
                                );
                            } else {
                                tracing::info!(
                                    target: "app_health_monitor",
                                    "MI BRIDGE: Logged fleet incident for {} ({} consecutive failures: {})",
                                    app_name, fail_count, error_msg
                                );
                            }
                        });
                    }
                }
            }

            // Phase 3: Auto-restart unhealthy apps via pm2
            for entry in &entries {
                maybe_restart_app(&state, &entry.app).await;
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

/// Get the consecutive failure count for a given app.
pub fn get_consecutive_failures(app: &str) -> u32 {
    CONSECUTIVE_FAILURES
        .lock()
        .ok()
        .and_then(|map| map.get(app).copied())
        .unwrap_or(0)
}

/// Probe a single app's health endpoint with semantic validation.
async fn probe_app(
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
                        if let (Some(avail), Some(expected)) = (pages_available, pages_expected) {
                            if avail < expected && status == "ok" {
                                status = "degraded".to_string();
                                error = Some(format!("Missing pages: {}/{} available", avail, expected));
                            }
                        }

                        // Response time SLA: slow response
                        if response_ms > RESPONSE_TIME_SLA_MS && status == "ok" {
                            status = "slow".to_string();
                            error = Some(format!("Response time {}ms exceeds {}ms SLA", response_ms, RESPONSE_TIME_SLA_MS));
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
    if let Some(deep) = deep_url {
        if entry.status == "ok" || entry.status == "slow" {
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
                    tracing::warn!(target: "app_health_monitor", "Deep probe failed for {}: {}", name, e);
                }
            }
        }
    }

    entry
}

/// Run a deep health probe against a `/api/health/deep` endpoint.
async fn probe_deep(client: &reqwest::Client, url: &str) -> Result<(bool, String), String> {
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

    let passed = json.get("healthy").and_then(|v| v.as_bool()).unwrap_or(false);
    let summary = json.get("summary").and_then(|v| v.as_str()).unwrap_or("no summary").to_string();

    Ok((passed, summary))
}

/// Handle WhatsApp alerting for a single app entry (cooldown + transition detection).
pub async fn handle_alert(state: &AppState, entry: &AppHealthEntry) {
    let prev = {
        let map = PREV_STATUS.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&entry.app).cloned().unwrap_or_else(|| "ok".to_string())
    };

    let is_bad = entry.status == "degraded" || entry.status == "unreachable" || entry.status == "slow";
    let was_bad = prev == "degraded" || prev == "unreachable" || prev == "slow";

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
                    _ => entry.error.clone().unwrap_or_else(|| "degraded response".to_string()),
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
        "INSERT INTO app_health_log (id, app, timestamp, status, pages_expected, pages_available, response_ms, error, semantic_status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&entry.app)
    .bind(&entry.last_checked)
    .bind(&entry.status)
    .bind(entry.pages_expected)
    .bind(entry.pages_available)
    .bind(entry.response_ms as i64)
    .bind(&entry.error)
    .bind(&entry.semantic_status)
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

/// Update the previous status for a given app (used by dependency_chain for batched alerts).
pub fn update_prev_status(app: &str, status: &str) {
    if let Ok(mut map) = PREV_STATUS.lock() {
        map.insert(app.to_string(), status.to_string());
    }
}

// ---- Phase 3: Server App Auto-Restart via pm2 ----

/// pm2 app name mapping (must match ecosystem.nextjs.config.cjs).
fn pm2_app_name(app: &str) -> Option<&'static str> {
    match app {
        "admin" => Some("rc-admin"),
        "kiosk" => Some("rc-kiosk"),
        "web" => Some("rc-web"),
        _ => None,
    }
}

/// Restart budget: max restarts per app per hour.
const MAX_RESTARTS_PER_HOUR: u32 = 2;
/// Restart cooldown auto-clear after this many seconds (1 hour).
const RESTART_COOLDOWN_SECS: u64 = 3600;
/// Consecutive "unreachable" cycles before triggering restart.
const RESTART_UNREACHABLE_THRESHOLD: u32 = 3;
/// Consecutive "degraded" cycles before triggering restart.
const RESTART_DEGRADED_THRESHOLD: u32 = 6;

struct RestartTracker {
    count: u32,
    first_restart_at: Instant,
    in_cooldown: bool,
}

static RESTART_TRACKERS: LazyLock<Mutex<HashMap<String, RestartTracker>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Check if an app should be restarted based on consecutive failures and restart budget.
pub async fn maybe_restart_app(state: &AppState, app: &str) {
    let failures = get_consecutive_failures(app);
    let pm2_name = match pm2_app_name(app) {
        Some(n) => n,
        None => return,
    };

    let should_restart = failures >= RESTART_UNREACHABLE_THRESHOLD
        || failures >= RESTART_DEGRADED_THRESHOLD;

    if !should_restart {
        return;
    }

    // Check restart budget
    let can_restart = {
        let mut trackers = RESTART_TRACKERS.lock().unwrap_or_else(|e| e.into_inner());
        let tracker = trackers.entry(app.to_string()).or_insert(RestartTracker {
            count: 0,
            first_restart_at: Instant::now(),
            in_cooldown: false,
        });

        // Auto-clear cooldown after RESTART_COOLDOWN_SECS
        if tracker.in_cooldown && tracker.first_restart_at.elapsed().as_secs() >= RESTART_COOLDOWN_SECS {
            tracker.count = 0;
            tracker.in_cooldown = false;
            tracing::info!(target: "app_health_monitor", "Restart cooldown cleared for {} -- budget reset", app);
        }

        if tracker.in_cooldown {
            false
        } else if tracker.count >= MAX_RESTARTS_PER_HOUR {
            tracker.in_cooldown = true;
            if state.config.alerting.enabled {
                let msg = format!(
                    "[APP RESTART] {} entered cooldown -- {} restarts exhausted, auto-clears in {}min. {}",
                    app, MAX_RESTARTS_PER_HOUR, RESTART_COOLDOWN_SECS / 60,
                    whatsapp_alerter::ist_now_string()
                );
                let config = state.config.clone();
                tokio::spawn(async move {
                    whatsapp_alerter::send_whatsapp(&config, &msg).await;
                });
            }
            false
        } else {
            tracker.count += 1;
            if tracker.count == 1 {
                tracker.first_restart_at = Instant::now();
            }
            true
        }
    };

    if !can_restart {
        return;
    }

    // Billing safety check before restarting kiosk
    if app == "kiosk" {
        if let Ok(active) = check_active_billing(state).await {
            if active {
                tracing::warn!(target: "app_health_monitor", "Skipping kiosk restart -- active billing sessions");
                if state.config.alerting.enabled {
                    let msg = format!(
                        "[APP RESTART] Kiosk restart DEFERRED -- active billing sessions. Staff should check. {}",
                        whatsapp_alerter::ist_now_string()
                    );
                    whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                }
                return;
            }
        }
    }

    tracing::warn!(target: "app_health_monitor", "Restarting {} (pm2: {}) after {} consecutive failures", app, pm2_name, failures);

    let output = tokio::process::Command::new("pm2")
        .arg("restart")
        .arg(pm2_name)
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let success = out.status.success();

            tracing::info!(target: "app_health_monitor", "pm2 restart {} -- success={}, stdout={}, stderr={}", pm2_name, success, stdout.trim(), stderr.trim());

            log_restart_to_db(&state.db, app, &format!("{}x failures", failures), if success { "success" } else { "pm2_error" }, &format!("{} {}", stdout.trim(), stderr.trim())).await;

            if state.config.alerting.enabled {
                let msg = format!(
                    "[APP RESTART] {} restarted via pm2 ({}). Result: {}. {}",
                    app, pm2_name, if success { "OK" } else { "FAILED" },
                    whatsapp_alerter::ist_now_string()
                );
                whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            }
        }
        Err(e) => {
            tracing::error!(target: "app_health_monitor", "Failed to execute pm2 restart for {}: {}", pm2_name, e);
            log_restart_to_db(&state.db, app, &format!("{}x failures", failures), "exec_error", &e.to_string()).await;
        }
    }
}

async fn check_active_billing(state: &AppState) -> Result<bool, String> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM billing_sessions WHERE status = 'active'")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(count.0 > 0)
}

async fn log_restart_to_db(db: &sqlx::SqlitePool, app: &str, trigger: &str, outcome: &str, pm2_stdout: &str) {
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = whatsapp_alerter::ist_now_string();
    let r = sqlx::query(
        "INSERT INTO app_restart_log (id, app, trigger, outcome, pm2_stdout, timestamp) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id).bind(app).bind(trigger).bind(outcome).bind(pm2_stdout).bind(&timestamp)
    .execute(db)
    .await;
    if let Err(e) = r {
        tracing::warn!(target: "app_health_monitor", "Failed to log restart for {}: {}", app, e);
    }
}