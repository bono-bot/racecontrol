//! Comms-link WebSocket integration (EG-08, EG-09).
//!
//! Provides:
//!   - Periodic heartbeat to James via comms-link WS (every 6h or on event)
//!   - GUARDIAN_ACTING coordination mutex via WS messages

use std::sync::Arc;

use crate::config::GuardianConfig;
use crate::GuardianState;
use serde::Serialize;
use tokio::sync::Mutex;
use tracing::{info, warn, debug, error};

/// Events sent to comms-link for James visibility.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum GuardianEvent {
    /// Periodic heartbeat — "I'm alive and watching"
    Heartbeat {
        build_id: String,
        uptime_secs: u64,
        server_status: String,
        consecutive_failures: u32,
        restart_count: u32,
    },
    /// Recovery deferred because another guardian is acting
    Deferred {
        reason: String,
        consecutive_failures: u32,
    },
    /// Restart succeeded
    RestartSuccess {
        method: String,
        consecutive_failures: u32,
    },
    /// Restart failed (both soft and hard)
    RestartFailed {
        consecutive_failures: u32,
    },
    /// Escalated as unsafe (active billing during peak)
    EscalatedUnsafe {
        consecutive_failures: u32,
    },
}

/// Comms-link message envelope.
#[derive(Debug, Serialize)]
struct CommsMessage {
    #[serde(rename = "type")]
    msg_type: String,
    sender: String,
    recipient: String,
    content: String,
    timestamp: String,
}

static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn uptime_secs() -> u64 {
    START_TIME
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs()
}

/// Send a guardian event to comms-link via HTTP POST to the gateway API.
///
/// Comms-link WS is complex to keep alive; we use the gateway REST API
/// which is always available on the same VPS.
pub async fn send_event(config: &GuardianConfig, event: &GuardianEvent) {
    let content = match serde_json::to_string(event) {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to serialize guardian event");
            return;
        }
    };

    let msg = CommsMessage {
        msg_type: "guardian_event".to_string(),
        sender: "bono-guardian".to_string(),
        recipient: "james".to_string(),
        content,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    // Try the gateway comms API first (most reliable on VPS)
    let gateway_url = "http://localhost:3100/api/comms/messages";

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to create HTTP client for comms");
            return;
        }
    };

    match client
        .post(gateway_url)
        .header("x-api-key", "rp-gateway-2026-secure-key")
        .header("Content-Type", "application/json")
        .json(&msg)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            debug!("Guardian event sent to comms gateway");
        }
        Ok(resp) => {
            warn!(
                status = resp.status().as_u16(),
                "Comms gateway returned non-success for guardian event"
            );
        }
        Err(e) => {
            warn!(error = %e, "Failed to send guardian event to comms gateway");
            // Fallback: try comms-link WS directly
            send_event_ws(config, event).await;
        }
    }
}

/// Fallback: send event via comms-link WebSocket.
async fn send_event_ws(config: &GuardianConfig, event: &GuardianEvent) {
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::Message;

    let content = match serde_json::to_string(event) {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to serialize event for WS");
            return;
        }
    };

    let ws_msg = serde_json::json!({
        "type": "guardian_event",
        "sender": "bono-guardian",
        "recipient": "james",
        "content": content,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    match tokio_tungstenite::connect_async(&config.comms_link_url).await {
        Ok((mut ws, _)) => {
            let msg_text = match serde_json::to_string(&ws_msg) {
                Ok(t) => t,
                Err(e) => {
                    error!(error = %e, "Failed to serialize WS message");
                    return;
                }
            };
            if let Err(e) = ws.send(Message::Text(msg_text.into())).await {
                warn!(error = %e, "Failed to send WS message");
            } else {
                debug!("Guardian event sent via WS fallback");
            }
            // Close gracefully
            let _ = ws.close(None).await;
        }
        Err(e) => {
            error!(error = %e, "Failed to connect to comms-link WS");
        }
    }
}

/// EG-08: Heartbeat loop — sends periodic heartbeats to comms-link.
///
/// Interval: every 6 hours (configurable), plus on any triggered event.
pub async fn heartbeat_loop(config: &GuardianConfig, state: &Arc<Mutex<GuardianState>>) {
    // Initialize start time
    let _ = START_TIME.get_or_init(std::time::Instant::now);

    let mut interval = tokio::time::interval(
        std::time::Duration::from_secs(config.heartbeat_interval_secs),
    );

    // Send initial heartbeat immediately
    {
        let guard = state.lock().await;
        let event = GuardianEvent::Heartbeat {
            build_id: crate::BUILD_ID.to_string(),
            uptime_secs: uptime_secs(),
            server_status: format!("{:?}", guard.current_status),
            consecutive_failures: guard.consecutive_failures,
            restart_count: guard.restart_count,
        };
        drop(guard);
        send_event(config, &event).await;
        info!("Initial heartbeat sent");
    }

    loop {
        interval.tick().await;

        let guard = state.lock().await;
        let event = GuardianEvent::Heartbeat {
            build_id: crate::BUILD_ID.to_string(),
            uptime_secs: uptime_secs(),
            server_status: format!("{:?}", guard.current_status),
            consecutive_failures: guard.consecutive_failures,
            restart_count: guard.restart_count,
        };
        drop(guard);

        send_event(config, &event).await;
        info!(uptime_secs = uptime_secs(), "Periodic heartbeat sent");
    }
}

/// EG-09: Try to acquire the GUARDIAN_ACTING coordination lock.
///
/// Sends a message to comms-link declaring that this guardian is taking action.
/// Returns `true` if we should proceed (optimistic — we always proceed unless
/// we see an explicit "already acting" response).
///
/// In the current implementation, this is a best-effort coordination mechanism.
/// The comms-link gateway stores the message for James's guardian to see.
/// True mutex would require a dedicated comms-link endpoint; for now we use
/// optimistic locking with a "last writer wins" approach.
pub async fn try_acquire_guardian_lock(_config: &GuardianConfig) -> bool {
    let msg = CommsMessage {
        msg_type: "guardian_acting".to_string(),
        sender: "bono-guardian".to_string(),
        recipient: "james-guardian".to_string(),
        content: serde_json::json!({
            "action": "acquire",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }).to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return true, // Can't check — proceed optimistically
    };

    // First, check if James guardian recently posted an "acquire"
    let check_url = "http://localhost:3100/api/comms/messages?sender=james-guardian&limit=1";
    match client
        .get(check_url)
        .header("x-api-key", "rp-gateway-2026-secure-key")
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if let Some(messages) = body.as_array() {
                    for msg_val in messages {
                        if let (Some(msg_type), Some(timestamp_str)) = (
                            msg_val.get("type").and_then(|v| v.as_str()),
                            msg_val.get("timestamp").and_then(|v| v.as_str()),
                        ) {
                            if msg_type == "guardian_acting" {
                                // Check if recent (within 5 minutes)
                                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
                                    let age = chrono::Utc::now() - ts.to_utc();
                                    if age.num_seconds() < 300 {
                                        info!(
                                            age_secs = age.num_seconds(),
                                            "James guardian is already acting — deferring"
                                        );
                                        return false;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(_) | Err(_) => {
            // Can't check — proceed optimistically
            debug!("Could not check James guardian status — proceeding");
        }
    }

    // Post our own acquire
    let gateway_url = "http://localhost:3100/api/comms/messages";
    match client
        .post(gateway_url)
        .header("x-api-key", "rp-gateway-2026-secure-key")
        .header("Content-Type", "application/json")
        .json(&msg)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            info!("GUARDIAN_ACTING lock acquired");
        }
        _ => {
            debug!("Could not post GUARDIAN_ACTING — proceeding anyway");
        }
    }

    true
}

/// EG-09: Release the GUARDIAN_ACTING lock.
pub async fn release_guardian_lock(config: &GuardianConfig) {
    let msg = CommsMessage {
        msg_type: "guardian_released".to_string(),
        sender: "bono-guardian".to_string(),
        recipient: "james-guardian".to_string(),
        content: serde_json::json!({
            "action": "release",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }).to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let gateway_url = "http://localhost:3100/api/comms/messages";
    match client
        .post(gateway_url)
        .header("x-api-key", "rp-gateway-2026-secure-key")
        .header("Content-Type", "application/json")
        .json(&msg)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            info!("GUARDIAN_ACTING lock released");
        }
        Ok(resp) => {
            warn!(status = resp.status().as_u16(), "Failed to release guardian lock");
        }
        Err(e) => {
            warn!(error = %e, "Failed to release guardian lock (network error)");
        }
    }

    // Suppress unused variable warning — config is used for future WS fallback
    let _ = config;
}
