//! rc-guardian: External Guardian (Layer 3 survival system)
//!
//! Runs on Bono's Linux VPS and watches the RaceControl server for failures.
//! Implements EG-01 through EG-10 requirements:
//!   - Health polling every 60s
//!   - Dead-man detection (3 consecutive misses = server dead)
//!   - Graduated restart via Tailscale SSH (soft -> hard -> report-only)
//!   - Billing safety check before restart
//!   - WhatsApp escalation when restart fails or is unsafe
//!   - Status classification (dead/busy/unreachable)
//!   - Guardian heartbeat to comms-link WebSocket
//!   - GUARDIAN_ACTING coordination mutex

mod config;
mod health;
mod restart;
mod alert;
mod comms;

use std::sync::Arc;
use chrono::Timelike;
use tokio::sync::Mutex;
use tracing::{info, warn, error};

const BUILD_ID: &str = env!("GIT_HASH");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(true)
        .with_thread_ids(false)
        .init();

    info!(build_id = BUILD_ID, "rc-guardian starting — Layer 3 External Guardian");

    // Load config
    let config = config::GuardianConfig::load()?;
    info!(
        server_url = %config.server_url,
        tailscale_ip = %config.tailscale_ip,
        poll_interval_secs = config.poll_interval_secs,
        "Configuration loaded"
    );

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(config.http_timeout_secs))
        .build()?;

    let state = Arc::new(Mutex::new(GuardianState::default()));

    // Spawn the comms-link heartbeat task (EG-08)
    let heartbeat_config = config.clone();
    let heartbeat_state = Arc::clone(&state);
    tokio::spawn(async move {
        info!("Heartbeat task started — interval {}s", heartbeat_config.heartbeat_interval_secs);
        comms::heartbeat_loop(&heartbeat_config, &heartbeat_state).await;
    });

    // Main health polling loop (EG-01)
    info!("Health poller started — interval {}s", config.poll_interval_secs);
    let mut interval = tokio::time::interval(
        std::time::Duration::from_secs(config.poll_interval_secs),
    );

    loop {
        interval.tick().await;

        let status = health::poll_server(&http_client, &config).await;
        let mut guard = state.lock().await;

        match &status {
            health::ServerStatus::Healthy { response_time_ms } => {
                if guard.consecutive_failures > 0 {
                    info!(
                        previous_failures = guard.consecutive_failures,
                        response_time_ms,
                        "Server recovered"
                    );
                }
                guard.consecutive_failures = 0;
                guard.last_healthy = Some(chrono::Utc::now());
                guard.current_status = StatusLabel::Healthy;
            }
            health::ServerStatus::Busy { response_time_ms } => {
                // EG-06: Busy = responding but slow. Don't count as failure.
                warn!(response_time_ms, "Server busy (slow response)");
                guard.current_status = StatusLabel::Busy;
                // Reset failure count — server IS responding
                guard.consecutive_failures = 0;
                guard.last_healthy = Some(chrono::Utc::now());
            }
            health::ServerStatus::Dead { error } => {
                guard.consecutive_failures += 1;
                guard.current_status = StatusLabel::Dead;
                warn!(
                    consecutive = guard.consecutive_failures,
                    error = %error,
                    "Server dead (connection refused)"
                );
            }
            health::ServerStatus::Unreachable { error } => {
                guard.consecutive_failures += 1;
                guard.current_status = StatusLabel::Unreachable;
                warn!(
                    consecutive = guard.consecutive_failures,
                    error = %error,
                    "Server unreachable (timeout)"
                );
            }
        }

        // EG-02: Dead-man detection — 3 consecutive misses
        if guard.consecutive_failures >= config.dead_man_threshold {
            let failures = guard.consecutive_failures;
            let status_label = guard.current_status;

            // Drop state lock before async recovery work
            drop(guard);

            error!(
                consecutive = failures,
                status = ?status_label,
                "DEAD-MAN TRIGGERED — {} consecutive failures",
                failures
            );

            // EG-09: Check GUARDIAN_ACTING coordination
            let acting = comms::try_acquire_guardian_lock(&config).await;
            if !acting {
                warn!("Another guardian is already acting — skipping recovery");
                // Send heartbeat about the event anyway
                let guard = state.lock().await;
                comms::send_event(&config, &comms::GuardianEvent::Deferred {
                    reason: "Another guardian already acting".to_string(),
                    consecutive_failures: failures,
                }).await;
                drop(guard);
                continue;
            }

            // EG-04: Billing safety check
            let billing_safe = health::check_billing_safety(&http_client, &config).await;

            if !billing_safe {
                warn!("Active billing sessions detected — restart is unsafe");

                // Check if peak hours
                let ist_hour = {
                    let utc_now = chrono::Utc::now();
                    // IST = UTC + 5:30
                    let ist = utc_now + chrono::Duration::hours(5) + chrono::Duration::minutes(30);
                    ist.hour()
                };
                let is_peak = (12..=22).contains(&ist_hour);

                if is_peak {
                    // EG-05: WhatsApp escalation for active sessions during peak
                    error!(
                        ist_hour,
                        "UNSAFE RESTART — active billing during peak hours, escalating to WhatsApp"
                    );
                    alert::send_whatsapp(
                        &http_client,
                        &config,
                        &format!(
                            "[rc-guardian] SERVER DOWN ({} consecutive failures) but active billing sessions exist during peak hours ({}:00 IST). Manual intervention required.",
                            failures, ist_hour
                        ),
                    ).await;

                    comms::send_event(&config, &comms::GuardianEvent::EscalatedUnsafe {
                        consecutive_failures: failures,
                        ist_hour,
                    }).await;

                    comms::release_guardian_lock(&config).await;
                    continue;
                }
                // Off-peak with billing: still escalate but less urgently
                warn!("Active billing during off-peak — escalating but less urgent");
                alert::send_whatsapp(
                    &http_client,
                    &config,
                    &format!(
                        "[rc-guardian] SERVER DOWN ({} consecutive failures). Active billing sessions exist (off-peak). Deferring restart. Check manually.",
                        failures
                    ),
                ).await;

                comms::release_guardian_lock(&config).await;
                continue;
            }

            // EG-07: Graduated restart
            info!("Billing check passed (no active sessions) — starting graduated restart");

            // Step 1: Soft restart via schtasks
            let soft_ok = restart::soft_restart(&config).await;
            if soft_ok {
                info!("Soft restart issued — waiting 30s for server to come back");
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;

                // Verify server is back
                let check = health::poll_server(&http_client, &config).await;
                if check.is_healthy() {
                    info!("Server recovered after soft restart");
                    let mut guard = state.lock().await;
                    guard.consecutive_failures = 0;
                    guard.current_status = StatusLabel::Healthy;
                    guard.last_healthy = Some(chrono::Utc::now());
                    guard.restart_count += 1;
                    drop(guard);

                    comms::send_event(&config, &comms::GuardianEvent::RestartSuccess {
                        method: "soft".to_string(),
                        consecutive_failures: failures,
                    }).await;

                    alert::send_whatsapp(
                        &http_client,
                        &config,
                        &format!(
                            "[rc-guardian] Server recovered after soft restart ({} failures before recovery).",
                            failures
                        ),
                    ).await;

                    comms::release_guardian_lock(&config).await;
                    continue;
                }

                warn!("Server still down after soft restart — trying hard restart");
            } else {
                warn!("Soft restart failed — trying hard restart");
            }

            // Step 2: Hard restart (taskkill + start)
            let hard_ok = restart::hard_restart(&config).await;
            if hard_ok {
                info!("Hard restart issued — waiting 45s for server to come back");
                tokio::time::sleep(std::time::Duration::from_secs(45)).await;

                let check = health::poll_server(&http_client, &config).await;
                if check.is_healthy() {
                    info!("Server recovered after hard restart");
                    let mut guard = state.lock().await;
                    guard.consecutive_failures = 0;
                    guard.current_status = StatusLabel::Healthy;
                    guard.last_healthy = Some(chrono::Utc::now());
                    guard.restart_count += 1;
                    drop(guard);

                    comms::send_event(&config, &comms::GuardianEvent::RestartSuccess {
                        method: "hard".to_string(),
                        consecutive_failures: failures,
                    }).await;

                    alert::send_whatsapp(
                        &http_client,
                        &config,
                        &format!(
                            "[rc-guardian] Server recovered after HARD restart ({} failures, soft restart failed).",
                            failures
                        ),
                    ).await;

                    comms::release_guardian_lock(&config).await;
                    continue;
                }
            }

            // Step 3: Report-only — both restarts failed
            error!("BOTH soft and hard restart failed — report-only mode");

            comms::send_event(&config, &comms::GuardianEvent::RestartFailed {
                consecutive_failures: failures,
            }).await;

            // EG-05: WhatsApp escalation on restart failure
            alert::send_whatsapp(
                &http_client,
                &config,
                &format!(
                    "[rc-guardian] CRITICAL: Server DOWN and BOTH restart attempts failed ({} consecutive failures). Soft restart: {}, Hard restart: {}. Immediate manual intervention required!",
                    failures,
                    if soft_ok { "issued but server didn't recover" } else { "SSH command failed" },
                    if hard_ok { "issued but server didn't recover" } else { "SSH command failed" },
                ),
            ).await;

            comms::release_guardian_lock(&config).await;
        }
    }
}

/// High-level status label for the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub enum StatusLabel {
    #[default]
    Unknown,
    Healthy,
    Busy,
    Dead,
    Unreachable,
}

/// Guardian's internal state.
#[derive(Debug, Default)]
pub struct GuardianState {
    pub consecutive_failures: u32,
    pub current_status: StatusLabel,
    pub last_healthy: Option<chrono::DateTime<chrono::Utc>>,
    pub restart_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_label_default() {
        let label = StatusLabel::default();
        assert_eq!(label, StatusLabel::Unknown);
    }

    #[test]
    fn test_guardian_state_default() {
        let state = GuardianState::default();
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.current_status, StatusLabel::Unknown);
        assert!(state.last_healthy.is_none());
        assert_eq!(state.restart_count, 0);
    }

    #[test]
    fn test_status_label_serialization() {
        let healthy = StatusLabel::Healthy;
        let json = serde_json::to_string(&healthy).expect("serialize");
        assert_eq!(json, "\"Healthy\"");

        let dead = StatusLabel::Dead;
        let json = serde_json::to_string(&dead).expect("serialize");
        assert_eq!(json, "\"Dead\"");
    }

    #[test]
    fn test_guardian_event_serialization() {
        let event = comms::GuardianEvent::Heartbeat {
            build_id: "abc123".into(),
            uptime_secs: 3600,
            server_status: "Healthy".into(),
            consecutive_failures: 0,
            restart_count: 0,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains("\"type\":\"Heartbeat\""));
        assert!(json.contains("\"build_id\":\"abc123\""));
    }

    #[test]
    fn test_guardian_event_deferred_serialization() {
        let event = comms::GuardianEvent::Deferred {
            reason: "Another guardian acting".into(),
            consecutive_failures: 3,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains("\"type\":\"Deferred\""));
        assert!(json.contains("\"consecutive_failures\":3"));
    }

    #[test]
    fn test_guardian_event_restart_success_serialization() {
        let event = comms::GuardianEvent::RestartSuccess {
            method: "soft".into(),
            consecutive_failures: 4,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains("\"type\":\"RestartSuccess\""));
        assert!(json.contains("\"method\":\"soft\""));
    }
}
