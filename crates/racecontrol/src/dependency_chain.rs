//! Dependency-Chain Awareness for App Health Alerting.
//!
//! The 3 Next.js apps (admin:3201, kiosk:3300, web:3200) all depend on the
//! RaceControl API (:8080). When the API is down, all 3 apps fail — but they
//! should produce a single root-cause alert, not 3 separate ones.
//!
//! This module intercepts health alert decisions and batches them when a
//! common upstream dependency is the root cause.

use crate::app_health_monitor::AppHealthEntry;
use crate::state::AppState;
use crate::whatsapp_alerter;

/// The upstream dependency that all apps share.
const API_UPSTREAM: &str = "api";
/// Apps that depend on the API upstream.
const DEPENDENT_APPS: &[&str] = &["admin", "kiosk", "web"];

/// Evaluate health entries with dependency-chain awareness and fire alerts.
///
/// If all dependent apps are failing AND the upstream API is also unhealthy,
/// fire a single root-cause alert instead of per-app alerts.
pub async fn evaluate_and_alert(state: &AppState, entries: &[AppHealthEntry]) {
    let api_down = is_api_down(state).await;

    let all_dependents_failing = DEPENDENT_APPS.iter().all(|app| {
        entries
            .iter()
            .any(|e| e.app == *app && (e.status == "degraded" || e.status == "unreachable"))
    });

    if api_down && all_dependents_failing {
        // Root-cause alert: API is down, all apps are failing
        handle_batched_alert(state, entries).await;
    } else {
        // Individual app alerts (normal path)
        for entry in entries {
            crate::app_health_monitor::handle_alert(state, entry).await;
        }
    }
}

/// Check if the core RaceControl API (:8080) is down.
/// We probe the server's own health endpoint from localhost.
async fn is_api_down(_state: &AppState) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    match client
        .get("http://127.0.0.1:8080/api/v1/health")
        .send()
        .await
    {
        Ok(resp) => !resp.status().is_success(),
        Err(_) => true,
    }
}

use std::sync::{LazyLock, Mutex};
use std::time::Instant;

/// Track whether we've already sent the batched "API DOWN" alert (cooldown).
static BATCHED_ALERT_COOLDOWN: LazyLock<Mutex<Option<Instant>>> =
    LazyLock::new(|| Mutex::new(None));

/// Previous batched state: true = was in "all down" state.
static PREV_BATCHED_STATE: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

/// Cooldown for batched alerts (5 minutes).
const BATCHED_COOLDOWN_SECS: u64 = 300;

/// Send a single batched alert for API-caused failures.
async fn handle_batched_alert(state: &AppState, entries: &[AppHealthEntry]) {
    let can_alert = {
        let guard = BATCHED_ALERT_COOLDOWN.lock().unwrap_or_else(|e| e.into_inner());
        match *guard {
            Some(last) => last.elapsed().as_secs() >= BATCHED_COOLDOWN_SECS,
            None => true,
        }
    };

    if can_alert {
        let app_statuses: Vec<String> = entries
            .iter()
            .map(|e| format!("{}={}", e.app, e.status))
            .collect();

        let msg = format!(
            "[APP HEALTH] API :8080 DOWN — all apps affected ({}). {}",
            app_statuses.join(", "),
            whatsapp_alerter::ist_now_string()
        );

        whatsapp_alerter::send_whatsapp(&state.config, &msg).await;

        if let Ok(mut guard) = BATCHED_ALERT_COOLDOWN.lock() {
            *guard = Some(Instant::now());
        }
    }

    // Update batched state
    if let Ok(mut guard) = PREV_BATCHED_STATE.lock() {
        *guard = true;
    }

    // Also update per-app PREV_STATUS so recovery detection works
    for entry in entries {
        crate::app_health_monitor::update_prev_status(&entry.app, &entry.status);
    }
}

/// Called from app_health_monitor when apps recover — sends batched recovery if needed.
pub async fn check_batched_recovery(state: &AppState, entries: &[AppHealthEntry]) {
    let was_batched = {
        PREV_BATCHED_STATE
            .lock()
            .ok()
            .map(|g| *g)
            .unwrap_or(false)
    };

    if !was_batched {
        return;
    }

    let all_ok = entries.iter().all(|e| e.status == "ok");

    if all_ok {
        let msg = format!(
            "[APP HEALTH] API :8080 RECOVERED — all apps back online. {}",
            whatsapp_alerter::ist_now_string()
        );
        whatsapp_alerter::send_whatsapp(&state.config, &msg).await;

        if let Ok(mut guard) = PREV_BATCHED_STATE.lock() {
            *guard = false;
        }
    }
}
