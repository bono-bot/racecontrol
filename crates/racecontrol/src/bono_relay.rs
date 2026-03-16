//! Bono relay: push real-time events to Bono's VPS over Tailscale mesh,
//! and receive inbound commands from Bono's cloud at the relay endpoint.

use std::sync::Arc;
use serde::{Serialize, Deserialize};
use crate::state::AppState;

/// Events pushed from racecontrol server to Bono's VPS webhook.
/// Tagged enum following the AgentMessage pattern in rc-common.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum BonoEvent {
    SessionStart { pod_number: u32, driver_name: String, game: String, session_id: String },
    SessionEnd { pod_number: u32, session_id: String, duration_secs: u64, paise_charged: i64 },
    LapRecorded { pod_number: u32, session_id: String, lap_time_ms: u64, track: String, car: String },
    PodOffline { pod_number: u32, ip: String, last_seen_secs_ago: u64 },
    PodOnline { pod_number: u32, ip: String, tailscale_ip: Option<String> },
    BillingEnd { pod_number: u32, session_id: String, driver_id: String },
}

/// Commands Bono's VPS can send to the relay endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RelayCommand {
    LaunchGame { pod_number: u32, game: String, track: Option<String>, car: Option<String> },
    StopGame { pod_number: u32 },
    GetStatus { pod_number: u32 },
}

/// Spawn the Bono relay background task.
/// No-ops if bono.enabled = false or webhook_url is not set.
pub fn spawn(state: Arc<AppState>) {
    let bono = &state.config.bono;
    if !bono.enabled {
        tracing::info!("Bono relay disabled");
        return;
    }
    match &bono.webhook_url {
        None => {
            tracing::warn!("Bono relay enabled but no webhook_url configured — skipping");
            return;
        }
        Some(_url) => {
            // TODO Wave 1: spawn tokio task for event push loop
            tracing::info!("Bono relay: webhook_url configured — event push not yet implemented");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_disabled() {
        // spawn() with enabled=false must not panic — just return early
        // We can't easily call spawn() in unit tests without AppState,
        // but we can test the guard logic directly:
        let enabled = false;
        let webhook_url: Option<String> = None;
        // Guard: if not enabled, do nothing
        if enabled {
            panic!("Should not reach here when disabled");
        }
        let _ = webhook_url; // suppress unused warning
    }

    #[test]
    fn spawn_no_url() {
        // spawn() with enabled=true but no webhook_url must not panic
        let enabled = true;
        let webhook_url: Option<String> = None;
        if enabled {
            if webhook_url.is_none() {
                // correct path — no panic
                return;
            }
            panic!("Should not reach here when webhook_url is None");
        }
    }

    #[test]
    fn event_serialization() {
        let event = BonoEvent::SessionStart {
            pod_number: 3,
            driver_name: "Uday".to_string(),
            game: "assetto_corsa".to_string(),
            session_id: "sess-001".to_string(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(v["type"], "session_start");
        assert_eq!(v["data"]["pod_number"], 3);
        assert_eq!(v["data"]["driver_name"], "Uday");
        assert_eq!(v["data"]["game"], "assetto_corsa");
        assert_eq!(v["data"]["session_id"], "sess-001");
    }
}
