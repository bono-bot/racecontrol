//! Subsystem health alert dispatch, dedup, and incident recording.

use std::time::{Duration, Instant};

use sqlx::SqlitePool;

use crate::config::{self, Config};

use super::{DEDUP_MAP, DEDUP_WINDOW_SECS, LOG_TARGET};

// ─── Dedup Logic (D3/OPS-04) ────────────────────────────────────────────────

/// Check if an alert should fire for this (subsystem, error_code) pair.
/// Returns false if the same pair was alerted within the last 10 minutes.
pub(super) fn should_alert(subsystem: &str, error_code: &str) -> bool {
    let key = (subsystem.to_string(), error_code.to_string());
    let mut map = DEDUP_MAP.lock().unwrap_or_else(|e| e.into_inner());

    // Evict old entries
    map.retain(|_, v| v.elapsed() < Duration::from_secs(DEDUP_WINDOW_SECS));

    if let Some(last) = map.get(&key)
        && last.elapsed() < Duration::from_secs(DEDUP_WINDOW_SECS) {
            return false; // suppressed
        }
    map.insert(key, Instant::now());
    true
}

// ─── Alert Dispatch (D2) — Fallback Chain: Direct -> Relay ─────────────────

/// Try direct Evolution API dispatch. Returns Ok(()) on success, Err(reason) on failure.
/// Used only by subsystem_health for the fallback chain — existing callers use send_whatsapp() unchanged.
async fn try_direct_whatsapp(config: &Config, message: &str) -> Result<(), String> {
    let (evo_url, evo_key, evo_instance, phone) = match (
        &config.auth.evolution_url,
        &config.auth.evolution_api_key,
        &config.auth.evolution_instance,
        &config.alerting.uday_phone,
    ) {
        (Some(u), Some(k), Some(i), Some(p)) if !u.is_empty() && !k.is_empty() => {
            (u.clone(), k.clone(), i.clone(), p.clone())
        }
        _ => return Err("Evolution API not configured".to_string()),
    };

    let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(&url)
        .header("apikey", &evo_key)
        .json(&serde_json::json!({
            "number": phone,
            "text": message
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("Evolution API returned {}", resp.status()))
    }
}

/// Fallback: POST to comms-link relay on James .27:8766
async fn try_relay_fallback(subsystem: &str, _error_code: &str, message: &str) -> Result<(), String> {
    let relay_url = "http://192.168.31.27:8766/relay/alert";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(relay_url)
        .json(&serde_json::json!({
            "source": "venue-racecontrol",
            "subsystem": subsystem,
            "severity": "critical",
            "message": message,
            "timestamp": crate::whatsapp_alerter::ist_now_string()
        }))
        .send()
        .await
        .map_err(|e| format!("Relay unreachable: {}", e))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("Relay returned {}", resp.status()))
    }
}

/// Fire a WhatsApp alert for a subsystem degradation.
/// Fallback chain per D2: try direct Evolution API first, fall back to comms-link relay.
pub(super) async fn dispatch_subsystem_alert(
    config: &Config,
    db: &SqlitePool,
    subsystem: &str,
    error_code: &str,
    detail: &str,
) {
    let is_cloud = config::this_instance_is_cloud(config);
    let server_label = if is_cloud { "cloud" } else { "venue (.23)" };

    let msg = format!(
        "[RP ALERT] Subsystem Degraded: {}\nError: {}{}\nServer: {} | {}",
        subsystem,
        error_code,
        if detail.is_empty() {
            String::new()
        } else {
            format!(" -- {}", detail)
        },
        server_label,
        crate::whatsapp_alerter::ist_now_string()
    );

    tracing::warn!(target: LOG_TARGET, subsystem, error_code, detail, "Subsystem degraded, sending alert");

    // Primary: direct Evolution API dispatch
    match try_direct_whatsapp(config, &msg).await {
        Ok(()) => {
            tracing::info!(target: LOG_TARGET, subsystem, "Alert sent via direct Evolution API");
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, subsystem, error = %e, "Direct WhatsApp failed, trying relay fallback");
            // Fallback: comms-link relay on James .27:8766
            match try_relay_fallback(subsystem, error_code, &msg).await {
                Ok(()) => {
                    tracing::info!(target: LOG_TARGET, subsystem, "Alert sent via relay fallback");
                }
                Err(e2) => {
                    tracing::error!(target: LOG_TARGET, subsystem, direct_error = %e, relay_error = %e2, "Both direct and relay failed for alert dispatch");
                }
            }
        }
    }

    // Always record incident regardless of dispatch success (D4/OPS-05)
    record_subsystem_incident(db, subsystem, error_code, &msg).await;
}

/// Fire a WhatsApp alert for subsystem recovery.
pub(super) async fn dispatch_recovery_alert(
    config: &Config,
    db: &SqlitePool,
    subsystem: &str,
    duration_secs: u64,
) {
    let msg = format!(
        "[RP RESOLVED] {} recovered (was down {}m {}s) | {}",
        subsystem,
        duration_secs / 60,
        duration_secs % 60,
        crate::whatsapp_alerter::ist_now_string()
    );

    tracing::info!(target: LOG_TARGET, subsystem, duration_secs, "Subsystem recovered, sending recovery alert");

    crate::whatsapp_alerter::send_whatsapp(config, &msg).await;

    // Resolve open incidents for this subsystem
    resolve_subsystem_incident(db, subsystem).await;
}

// ─── Incident Recording (D4/OPS-05) ────────────────────────────────────────

/// Record a subsystem degradation incident in alert_incidents table.
async fn record_subsystem_incident(
    db: &SqlitePool,
    subsystem: &str,
    _error_code: &str,
    message: &str,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let corr_id = uuid::Uuid::new_v4().to_string();

    if let Err(e) = sqlx::query(
        "INSERT INTO alert_incidents (id, alert_type, subsystem, severity, description, correlation_id)
         VALUES (?, 'subsystem_degraded', ?, 'critical', ?, ?)",
    )
    .bind(&id)
    .bind(subsystem)
    .bind(message)
    .bind(&corr_id)
    .execute(db)
    .await
    {
        tracing::warn!(target: LOG_TARGET, error = %e, subsystem, "Failed to record subsystem incident");
    }
}

/// Resolve open incidents for a subsystem.
async fn resolve_subsystem_incident(db: &SqlitePool, subsystem: &str) {
    if let Err(e) = sqlx::query(
        "UPDATE alert_incidents SET resolved_at = datetime('now')
         WHERE subsystem = ? AND resolved_at IS NULL AND alert_type = 'subsystem_degraded'",
    )
    .bind(subsystem)
    .execute(db)
    .await
    {
        tracing::warn!(target: LOG_TARGET, error = %e, subsystem, "Failed to resolve subsystem incidents");
    }
}
