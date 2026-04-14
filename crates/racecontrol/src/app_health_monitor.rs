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

#[path = "app_health_probes.rs"]
mod probes;

#[path = "app_health_restart.rs"]
mod restart;

/// Response time SLA threshold (ms). Above this → "slow" status.
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
    /// Semantic status from deep health probe (Phase 1.4).
    /// None if deep probe hasn't run yet or isn't applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_status: Option<String>,
    /// Whether the deep health probe passed (Phase 1.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deep_check_passed: Option<bool>,
}

/// App targets to probe (name, health URL, deep health URL).
/// Includes both venue (LAN) and cloud (public) frontends.
const APP_TARGETS: &[(&str, &str, Option<&str>)] = &[
    // Venue apps (LAN)
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
    // Cloud apps (MI Gap 3: cloud frontend health probes)
    ("cloud-admin", "https://admin.racingpoint.cloud/api/health", None),
    ("cloud-app", "https://app.racingpoint.cloud", None),
    ("cloud-web", "https://racingpoint.cloud", None),
];

/// Critical pages to probe — if ANY returns 404, the app is broken even if /api/health says "ok".
/// Checked every deep-probe cycle (not every 30s — reduces load).
/// Format: (app_name, page_url, expected_status_code)
const CRITICAL_PAGE_PROBES: &[(&str, &str, u16)] = &[
    // Venue pages
    ("kiosk", "http://192.168.31.23:3300/kiosk", 200),           // main lock screen
    ("kiosk", "http://192.168.31.23:3300/kiosk/staff", 200),     // staff login (was 307 before middleware fix ea0f6f82)
    ("kiosk", "http://192.168.31.23:3300/kiosk/spectator", 200), // spectator display
    ("web", "http://192.168.31.23:3200/spectator/circuit", 200),  // Phase 335: spectator circuit viewer
    ("web", "http://192.168.31.23:3200/billing", 200),            // POS billing page
    ("admin", "http://192.168.31.23:3201/pods", 200),             // admin pod view
    // Cloud pages (MI Gap 3: cloud frontend page probes)
    ("cloud-app", "https://app.racingpoint.cloud", 200),          // customer PWA
    ("cloud-admin", "https://admin.racingpoint.cloud", 200),      // admin dashboard
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

            // Reset WS churn counters every 60s (every 2nd cycle)
            if cycle_count % 2 == 0 {
                let (connects, disconnects) = crate::ws::dashboard_ws_churn();
                if connects > 10 || disconnects > 10 {
                    tracing::warn!(
                        target: "app_health_monitor",
                        "Dashboard WS churn detected: {connects} connects, {disconnects} disconnects in 60s — stale frontend build?"
                    );
                }
                crate::ws::reset_ws_churn_counters();
            }

            // Probe all 3 apps concurrently
            let (admin, kiosk, web) = tokio::join!(
                probes::probe_app(&client, APP_TARGETS[0].0, APP_TARGETS[0].1, if run_deep { APP_TARGETS[0].2 } else { None }),
                probes::probe_app(&client, APP_TARGETS[1].0, APP_TARGETS[1].1, if run_deep { APP_TARGETS[1].2 } else { None }),
                probes::probe_app(&client, APP_TARGETS[2].0, APP_TARGETS[2].1, if run_deep { APP_TARGETS[2].2 } else { None }),
            );

            let entries = vec![admin, kiosk, web];

            // Critical page probes — run during deep-probe cycles only
            if run_deep {
                for (app_name, page_url, expected_status) in CRITICAL_PAGE_PROBES {
                    match client.get(*page_url).send().await {
                        Ok(resp) => {
                            let status = resp.status().as_u16();
                            if status == 404 {
                                tracing::error!(
                                    target: "app_health_monitor",
                                    "CRITICAL PAGE FAILURE: {app_name} page {page_url} returned 404 — \
                                     page is missing from deploy. Portal health says 'ok' but the page \
                                     is unreachable for users."
                                );
                                // Log as MI incident
                                let _ = sqlx::query(
                                    "INSERT OR IGNORE INTO fleet_incidents \
                                     (id, node, problem_key, severity, detected_at) \
                                     VALUES (?, 'server', ?, 'high', datetime('now'))"
                                )
                                .bind(format!("inc_page_{}_{}", app_name, chrono::Utc::now().timestamp()))
                                .bind(format!("frontend_page_failure:{}", app_name))
                                .execute(&state.db)
                                .await;
                            } else if status != *expected_status && status != 200 && status != 307 {
                                tracing::warn!(
                                    target: "app_health_monitor",
                                    "Page probe {app_name} {page_url}: expected {expected_status}, got {status}"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                target: "app_health_monitor",
                                "Page probe {app_name} {page_url} failed: {e}"
                            );
                        }
                    }
                }
            }

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
            // MMA P1 fixes: nanosecond IDs, incident resolution on recovery, poison logging.
            {
                // MMA P2: Log poisoned mutex instead of silent skip
                let counts: Vec<(String, u32)> = match CONSECUTIVE_FAILURES.lock() {
                    Ok(map) => entries.iter().map(|e| {
                        (e.app.clone(), map.get(&e.app).copied().unwrap_or(0))
                    }).collect(),
                    Err(poisoned) => {
                        tracing::error!(
                            target: "app_health_monitor",
                            "CONSECUTIVE_FAILURES mutex poisoned — MI bridge disabled this cycle: {}",
                            poisoned
                        );
                        vec![]
                    }
                };
                // MMA P2: Lock dropped here — no allocations or spawns while held

                for (app_name, count) in &counts {
                    // Emit incident at 5 consecutive failures, then every 20th after that
                    if *count == 5 || (*count > 5 && *count % 20 == 0) {
                        // MMA P1: Use nanosecond timestamp for unique IDs
                        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
                        let incident = rc_common::mesh_types::MeshIncident {
                            id: format!("inc_app_{}_{}", app_name, nanos),
                            node: "server".to_string(),
                            problem_key: format!("app_degraded:{}", app_name),
                            severity: if *count >= 20 {
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
                        let app = app_name.clone();
                        let error_msg = entries.iter()
                            .find(|e| e.app == *app_name)
                            .and_then(|e| e.error.clone())
                            .unwrap_or_default();
                        let fail_count = *count;
                        tokio::spawn(async move {
                            if let Err(e) = crate::fleet_kb::insert_incident(&db, &incident).await {
                                tracing::warn!(
                                    target: "app_health_monitor",
                                    "Failed to log MI incident for {}: {}", app, e
                                );
                            } else {
                                tracing::info!(
                                    target: "app_health_monitor",
                                    "MI BRIDGE: Logged fleet incident for {} ({} consecutive failures: {})",
                                    app, fail_count, error_msg
                                );
                            }
                        });
                    }

                    // MMA P1: Resolve incident when app recovers (count drops to 0)
                    if *count == 0 {
                        let db = state.db.clone();
                        let problem_key = format!("app_degraded:{}", app_name);
                        tokio::spawn(async move {
                            // Close any open incidents for this app
                            let now = chrono::Utc::now().to_rfc3339();
                            let result = sqlx::query(
                                "UPDATE fleet_incidents SET resolved_at = ?1, resolution = 'auto_recovered'
                                 WHERE problem_key = ?2 AND resolved_at IS NULL"
                            )
                            .bind(&now)
                            .bind(&problem_key)
                            .execute(&db)
                            .await;
                            if let Ok(r) = result {
                                if r.rows_affected() > 0 {
                                    tracing::info!(
                                        target: "app_health_monitor",
                                        "MI BRIDGE: Resolved {} open incident(s) for {}",
                                        r.rows_affected(), problem_key
                                    );
                                }
                            }
                        });
                    }
                }
            }

            // Phase 3: Auto-restart unhealthy apps via pm2
            for entry in &entries {
                restart::maybe_restart_app(&state, &entry.app).await;
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

/// Handle WhatsApp alerting for a single app entry (cooldown + transition detection).
/// Public for use by dependency_chain module.
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
/// Phase 1.5: Includes semantic_status column.
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
