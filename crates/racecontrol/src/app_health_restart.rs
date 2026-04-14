//! Phase 3: Server App Auto-Restart via pm2.
//! Extracted from app_health_monitor.rs for ARCH-03 (<500 line modules).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use crate::state::AppState;
use crate::whatsapp_alerter;

use super::get_consecutive_failures;

/// pm2 app name mapping (must match ecosystem.nextjs.config.cjs).
fn pm2_app_name(app: &str) -> Option<&'static str> {
    match app {
        "admin" => Some("rc-admin"),
        "kiosk" => Some("rc-kiosk"),
        "web" => Some("rc-web"),
        _ => None,
    }
}

/// Cloud app pm2 names — restarted via comms-link relay to Bono VPS.
fn cloud_pm2_app_name(app: &str) -> Option<&'static str> {
    match app {
        "cloud-admin" => Some("racingpoint-admin"),
        "cloud-app" => Some("racecontrol-pwa"),
        "cloud-web" => Some("racingpoint-web"),
        _ => None,
    }
}

/// Restart a cloud app via comms-link relay exec (Bono VPS pm2 restart).
async fn restart_cloud_app(state: &AppState, app: &str) {
    let pm2_name = match cloud_pm2_app_name(app) {
        Some(n) => n,
        None => return,
    };

    tracing::info!(target: "app_health_monitor", "Cloud auto-restart: pm2 restart {} via relay", pm2_name);

    // Use comms-link relay to execute pm2 restart on Bono VPS
    let relay_url = "http://localhost:8766/relay/exec/run";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    let body = serde_json::json!({
        "command": "custom",
        "args": format!("pm2 restart {}", pm2_name),
        "reason": format!("MI auto-restart: {} unhealthy", app)
    });

    match client.post(relay_url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(target: "app_health_monitor",
                "Cloud auto-restart SUCCESS: pm2 restart {} via relay", pm2_name
            );
            let msg = format!("☁️ Cloud app {} restarted by MI (was unhealthy)", app);
            crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
        }
        Ok(resp) => {
            tracing::warn!(target: "app_health_monitor",
                "Cloud auto-restart FAILED: relay returned {}", resp.status()
            );
        }
        Err(e) => {
            tracing::warn!(target: "app_health_monitor",
                "Cloud auto-restart FAILED: relay error — {}", e
            );
        }
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

/// Per-app restart tracking.
struct RestartTracker {
    count: u32,
    first_restart_at: Instant,
    in_cooldown: bool,
}

static RESTART_TRACKERS: LazyLock<Mutex<HashMap<String, RestartTracker>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Check if an app should be restarted based on consecutive failures and restart budget.
/// Called from the main probe loop.
pub(crate) async fn maybe_restart_app(state: &AppState, app: &str) {
    let failures = get_consecutive_failures(app);

    // Cloud apps: restart via comms-link relay instead of local pm2
    if cloud_pm2_app_name(app).is_some() {
        if failures >= RESTART_UNREACHABLE_THRESHOLD {
            restart_cloud_app(state, app).await;
        }
        return;
    }

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
            tracing::info!(
                target: "app_health_monitor",
                "Restart cooldown cleared for {} — budget reset", app
            );
        }

        if tracker.in_cooldown {
            false
        } else if tracker.count >= MAX_RESTARTS_PER_HOUR {
            tracker.in_cooldown = true;
            // Alert on cooldown entry
            if state.config.alerting.enabled {
                let msg = format!(
                    "[APP RESTART] {} entered cooldown — {} restarts exhausted, auto-clears in {}min. {}",
                    app, MAX_RESTARTS_PER_HOUR, RESTART_COOLDOWN_SECS / 60,
                    whatsapp_alerter::ist_now_string()
                );
                // Fire-and-forget: we're inside a sync lock
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
                tracing::warn!(
                    target: "app_health_monitor",
                    "Skipping kiosk restart — active billing sessions detected"
                );
                if state.config.alerting.enabled {
                    let msg = format!(
                        "[APP RESTART] Kiosk restart DEFERRED — active billing sessions. Staff should check kiosk. {}",
                        whatsapp_alerter::ist_now_string()
                    );
                    whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
                }
                return;
            }
        }
    }

    // Execute restart via pm2
    tracing::warn!(
        target: "app_health_monitor",
        "Restarting {} (pm2: {}) after {} consecutive failures",
        app, pm2_name, failures
    );

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

            tracing::info!(
                target: "app_health_monitor",
                "pm2 restart {} — success={}, stdout={}, stderr={}",
                pm2_name, success, stdout.trim(), stderr.trim()
            );

            // Log restart attempt to DB
            log_restart_to_db(
                &state.db,
                app,
                &format!("{}x failures", failures),
                if success { "success" } else { "pm2_error" },
                &format!("{} {}", stdout.trim(), stderr.trim()),
            )
            .await;

            if state.config.alerting.enabled {
                let msg = format!(
                    "[APP RESTART] {} restarted via pm2 ({}). Result: {}. {}",
                    app,
                    pm2_name,
                    if success { "OK" } else { "FAILED" },
                    whatsapp_alerter::ist_now_string()
                );
                whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            }
        }
        Err(e) => {
            tracing::error!(
                target: "app_health_monitor",
                "Failed to execute pm2 restart for {}: {}", pm2_name, e
            );
            log_restart_to_db(&state.db, app, &format!("{}x failures", failures), "exec_error", &e.to_string()).await;
        }
    }
}

/// Check if there are active billing sessions.
async fn check_active_billing(state: &AppState) -> Result<bool, String> {
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM billing_sessions WHERE status = 'active'",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    Ok(count.0 > 0)
}

/// Log a restart attempt to the app_restart_log table.
async fn log_restart_to_db(
    db: &sqlx::SqlitePool,
    app: &str,
    trigger: &str,
    outcome: &str,
    pm2_stdout: &str,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = whatsapp_alerter::ist_now_string();
    let r = sqlx::query(
        "INSERT INTO app_restart_log (id, app, trigger, outcome, pm2_stdout, timestamp) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(app)
    .bind(trigger)
    .bind(outcome)
    .bind(pm2_stdout)
    .bind(&timestamp)
    .execute(db)
    .await;

    if let Err(e) = r {
        tracing::warn!(target: "app_health_monitor", "Failed to log restart for {}: {}", app, e);
    }
}
