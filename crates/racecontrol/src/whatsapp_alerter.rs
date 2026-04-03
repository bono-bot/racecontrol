//! WhatsApp P0 alerter — sends critical alerts and recovery notifications to Uday
//! via Evolution API. Monitors bono_event_tx for pod events and error_rate broadcast
//! channel for error spikes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sqlx::SqlitePool;
use tokio::sync::broadcast;

use crate::config::Config;
use crate::bono_relay::BonoEvent;
use crate::state::AppState;

/// Internal P0 tracking state (NOT shared with AppState).
struct P0State {
    all_pods_offline_since: Option<Instant>,
    last_all_pods_alert: Option<Instant>,
    error_rate_since: Option<Instant>,
    last_error_rate_alert: Option<Instant>,
    error_rate_last_signal: Option<Instant>,
    /// Tracks when we last checked PIN rotation age (once per 24h).
    last_pin_rotation_check: Option<Instant>,
}

impl P0State {
    fn new() -> Self {
        Self {
            all_pods_offline_since: None,
            last_all_pods_alert: None,
            error_rate_since: None,
            last_error_rate_alert: None,
            error_rate_last_signal: None,
            last_pin_rotation_check: None,
        }
    }
}

/// Returns IST timestamp string (e.g., "20 Mar 2026 15:30 IST").
pub(crate) fn ist_now_string() -> String {
    chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Kolkata)
        .format("%d %b %Y %H:%M IST")
        .to_string()
}

/// Returns (online_count, total_count).
/// Online = present in agent_senders AND sender is not closed.
/// Total = pods map length.
async fn count_online_pods(state: &Arc<AppState>) -> (usize, usize) {
    let agent_senders = state.agent_senders.read().await;
    let pods = state.pods.read().await;
    let online = agent_senders.values().filter(|s| !s.is_closed()).count();
    let total = pods.len();
    (online, total)
}

/// Send WhatsApp message via Evolution API. Best-effort: warns on failure, never panics.
pub(crate) async fn send_whatsapp(config: &Config, message: &str) {
    let (evo_url, evo_key, evo_instance, phone) = match (
        &config.auth.evolution_url,
        &config.auth.evolution_api_key,
        &config.auth.evolution_instance,
        &config.alerting.uday_phone,
    ) {
        (Some(url), Some(key), Some(inst), Some(phone)) => (url, key, inst, phone),
        _ => {
            tracing::warn!(target: "whatsapp_alerter", "Evolution API or uday_phone not configured, skipping WA alert");
            return;
        }
    };

    let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
    let body = serde_json::json!({
        "number": phone,
        "text": message
    });

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: "whatsapp_alerter", "Failed to build HTTP client: {}", e);
            return;
        }
    };

    match client.post(&url).header("apikey", evo_key).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(target: "whatsapp_alerter", "WA P0 alert sent to {}", phone);
        }
        Ok(resp) => {
            tracing::warn!(target: "whatsapp_alerter", "Evolution API returned {} for WA alert", resp.status());
        }
        Err(e) => {
            tracing::warn!(target: "whatsapp_alerter", "WA alert send failed: {}", e);
        }
    }
}

/// Send a WhatsApp message to a specific phone number (not just Uday).
/// Used for staff PIN rotation and other per-person notifications.
pub async fn send_whatsapp_to(config: &Config, phone: &str, message: &str) {
    let (evo_url, evo_key, evo_instance) = match (
        &config.auth.evolution_url,
        &config.auth.evolution_api_key,
        &config.auth.evolution_instance,
    ) {
        (Some(url), Some(key), Some(inst)) => (url, key, inst),
        _ => {
            tracing::warn!(target: "whatsapp_alerter", "Evolution API not configured, skipping WA message to {}", phone);
            return;
        }
    };

    let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
    let body = serde_json::json!({
        "number": phone,
        "text": message
    });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: "whatsapp_alerter", "Failed to build HTTP client: {}", e);
            return;
        }
    };

    match client.post(&url).header("apikey", evo_key).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(target: "whatsapp_alerter", "WA message sent to {}", phone);
        }
        Ok(resp) => {
            tracing::warn!(target: "whatsapp_alerter", "Evolution API returned {} for WA message to {}", resp.status(), phone);
        }
        Err(e) => {
            tracing::warn!(target: "whatsapp_alerter", "WA message send to {} failed: {}", phone, e);
        }
    }
}

/// Send a WhatsApp alert for sensitive admin actions (login, topup, fleet exec).
/// Best-effort: uses existing send_whatsapp() + ist_now_string().
pub(crate) async fn send_admin_alert(config: &Config, action: &str, details: &str) {
    let msg = format!("[ADMIN] {} -- {} | {}", action, details, ist_now_string());
    send_whatsapp(config, &msg).await;
}

/// Record a new incident in the alert_incidents table.
async fn record_incident(db: &SqlitePool, alert_type: &str, pod_count: Option<i64>, description: &str) {
    let id = uuid::Uuid::new_v4().to_string();
    let result = sqlx::query(
        "INSERT INTO alert_incidents (id, alert_type, started_at, pod_count, description)
         VALUES (?, ?, datetime('now'), ?, ?)"
    )
    .bind(&id)
    .bind(alert_type)
    .bind(pod_count)
    .bind(description)
    .execute(db)
    .await;

    if let Err(e) = result {
        tracing::warn!(target: "whatsapp_alerter", "Failed to record incident: {}", e);
    }
}

/// Resolve the most recent unresolved incident of a given type.
async fn resolve_incident(db: &SqlitePool, alert_type: &str) {
    let result = sqlx::query(
        "UPDATE alert_incidents SET resolved_at = datetime('now')
         WHERE alert_type = ? AND resolved_at IS NULL"
    )
    .bind(alert_type)
    .execute(db)
    .await;

    if let Err(e) = result {
        tracing::warn!(target: "whatsapp_alerter", "Failed to resolve incident: {}", e);
    }
}

/// Per-pod debounce map for security alerts (5 min cooldown per pod).
static SECURITY_ALERT_DEBOUNCE: std::sync::LazyLock<Mutex<HashMap<String, Instant>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

const SECURITY_ALERT_COOLDOWN_SECS: u64 = 300; // 5 minutes per pod

/// Send a security alert via WhatsApp with per-pod debounce (5 min cooldown).
/// Called from ws/mod.rs on KioskLockdown events.
pub(crate) async fn send_security_alert(config: &Config, pod_id: &str, message: &str) {
    // Debounce check
    {
        let mut map = SECURITY_ALERT_DEBOUNCE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(last) = map.get(pod_id) {
            if last.elapsed().as_secs() < SECURITY_ALERT_COOLDOWN_SECS {
                tracing::debug!(target: "whatsapp_alerter", "Security alert for pod {} suppressed (cooldown)", pod_id);
                return;
            }
        }
        map.insert(pod_id.to_string(), Instant::now());
    }

    send_whatsapp(config, message).await;
}

/// Main WhatsApp alerter task. Subscribes to bono_event_tx for pod events
/// and error_rate broadcast for error spikes. Fires P0 alerts and resolved
/// notifications to Uday via Evolution API.
pub async fn whatsapp_alerter_task(
    state: Arc<AppState>,
    mut error_rate_rx: broadcast::Receiver<()>,
) {
    if !state.config.alerting.enabled {
        tracing::info!(target: "whatsapp_alerter", "WA alerting disabled");
        return;
    }

    if state.config.alerting.uday_phone.is_none() {
        tracing::warn!(target: "whatsapp_alerter", "No uday_phone configured, WA alerting disabled");
        return;
    }

    let cooldown = Duration::from_secs(state.config.alerting.cooldown_secs);
    let mut bono_rx = state.bono_event_tx.subscribe();
    let mut p0 = P0State::new();

    tracing::info!(target: "whatsapp_alerter", "WA P0 alerter started (cooldown={}s)", state.config.alerting.cooldown_secs);

    loop {
        tokio::select! {
            event = bono_rx.recv() => {
                match event {
                    Ok(BonoEvent::PodOffline { .. }) => {
                        // Debounce: wait 2s for cascading disconnects
                        tokio::time::sleep(Duration::from_secs(2)).await;

                        let (online, total) = count_online_pods(&state).await;
                        if online == 0 && total > 0 {
                            let can_alert = match p0.last_all_pods_alert {
                                None => true,
                                Some(last) => last.elapsed() > cooldown,
                            };
                            if can_alert {
                                let msg = format!(
                                    "[RP ALERT] All Pods Offline - All {} pods lost WS connection. {} pods affected. {}",
                                    total, total, ist_now_string()
                                );
                                send_whatsapp(&state.config, &msg).await;
                                p0.all_pods_offline_since = Some(Instant::now());
                                p0.last_all_pods_alert = Some(Instant::now());
                                record_incident(&state.db, "all_pods_offline", Some(total as i64), "All pods lost WebSocket connection").await;
                            }
                        }
                    }
                    Ok(BonoEvent::PodOnline { .. }) => {
                        let (online, total) = count_online_pods(&state).await;
                        if online == total && total > 0 && p0.all_pods_offline_since.is_some() {
                            let duration_mins = p0.all_pods_offline_since.unwrap().elapsed().as_secs() / 60;
                            let msg = format!(
                                "[RP RESOLVED] All Pods Offline cleared. All {} pods online. Duration: {}m. {}",
                                total, duration_mins, ist_now_string()
                            );
                            send_whatsapp(&state.config, &msg).await;
                            resolve_incident(&state.db, "all_pods_offline").await;
                            p0.all_pods_offline_since = None;
                        }
                    }
                    Ok(_) => {} // ignore other events
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(target: "whatsapp_alerter", "Bono event rx lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!(target: "whatsapp_alerter", "Bono event channel closed, shutting down");
                        break;
                    }
                }
            }

            result = error_rate_rx.recv() => {
                match result {
                    Ok(()) => {
                        let can_alert = match p0.last_error_rate_alert {
                            None => true,
                            Some(last) => last.elapsed() > cooldown,
                        };
                        if can_alert {
                            let msg = format!(
                                "[RP ALERT] High Error Rate - Error rate threshold exceeded on racecontrol. {}",
                                ist_now_string()
                            );
                            send_whatsapp(&state.config, &msg).await;
                            p0.error_rate_since = Some(Instant::now());
                            p0.last_error_rate_alert = Some(Instant::now());
                            p0.error_rate_last_signal = Some(Instant::now());
                            record_incident(&state.db, "error_rate", None, "Error rate threshold breach").await;
                        } else {
                            // Still receiving signals — update last_signal for resolved check
                            p0.error_rate_last_signal = Some(Instant::now());
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(target: "whatsapp_alerter", "Error rate rx lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!(target: "whatsapp_alerter", "Error rate channel closed, shutting down");
                        break;
                    }
                }
            }

            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                // Periodic check: resolve error rate if no signal for 5 minutes
                if let Some(last_signal) = p0.error_rate_last_signal {
                    if p0.error_rate_since.is_some() && last_signal.elapsed() > Duration::from_secs(300) {
                        let duration_mins = p0.error_rate_since.unwrap().elapsed().as_secs() / 60;
                        let msg = format!(
                            "[RP RESOLVED] High Error Rate cleared. No threshold breach for 5 minutes. Duration: {}m. {}",
                            duration_mins, ist_now_string()
                        );
                        send_whatsapp(&state.config, &msg).await;
                        resolve_incident(&state.db, "error_rate").await;
                        p0.error_rate_since = None;
                        p0.error_rate_last_signal = None;
                    }
                }

                // PIN rotation check: once per 24 hours (ADMIN-06)
                let should_check_pin = p0.last_pin_rotation_check
                    .map_or(true, |t| t.elapsed() > Duration::from_secs(86400));
                if should_check_pin {
                    check_pin_rotation_age(&state, &mut p0).await;
                }
            }
        }
    }
}

/// Check if the admin PIN has not been changed in 30+ days and alert Uday.
async fn check_pin_rotation_age(state: &Arc<AppState>, p0: &mut P0State) {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT updated_at FROM system_settings WHERE key = 'admin_pin_hash_sha256'",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((updated_at,)) = row {
        // Parse SQLite datetime format "YYYY-MM-DD HH:MM:SS"
        if let Ok(changed_at) = chrono::NaiveDateTime::parse_from_str(&updated_at, "%Y-%m-%d %H:%M:%S") {
            let now = chrono::Utc::now().naive_utc();
            let days_since = (now - changed_at).num_days();
            if days_since > 30 {
                let msg = format!(
                    "[SECURITY] Staff PIN has not been changed in {} days. Please update your admin PIN. {}",
                    days_since, ist_now_string()
                );
                send_whatsapp(&state.config, &msg).await;
                tracing::warn!(target: "whatsapp_alerter", "PIN rotation alert: {} days since last change", days_since);
            } else {
                tracing::debug!(target: "whatsapp_alerter", "PIN rotation OK: changed {} days ago", days_since);
            }
        }
    }

    p0.last_pin_rotation_check = Some(Instant::now());
}
