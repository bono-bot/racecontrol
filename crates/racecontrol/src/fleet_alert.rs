//! Fleet alert API -- Tier 4 WhatsApp escalation endpoint.
//!
//! POST /api/v1/fleet/alert -- accepts {pod_id, message, severity} and sends
//! a WhatsApp alert to staff via whatsapp_alerter::send_admin_alert().
//! Public route (no auth) -- rc-sentry calls this from pods without JWT.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

use crate::state::AppState;
use crate::whatsapp_alerter;

/// Request body for POST /api/v1/fleet/alert.
#[derive(Debug, Deserialize)]
pub struct FleetAlertRequest {
    /// Pod identifier (e.g. "pod-8")
    pub pod_id: String,
    /// Human-readable alert message
    pub message: String,
    /// Severity level: "info", "warning", "critical"
    pub severity: String,
}

/// POST /api/v1/fleet/alert -- send a WhatsApp alert to staff.
///
/// Returns 202 Accepted (fire-and-forget -- WhatsApp delivery is best-effort).
/// Used by rc-sentry Tier 4 escalation after 3+ failed recovery attempts.
pub async fn post_fleet_alert(
    State(state): State<Arc<AppState>>,
    Json(req): Json<FleetAlertRequest>,
) -> StatusCode {
    tracing::warn!(
        target: "fleet_alert",
        pod_id = %req.pod_id,
        severity = %req.severity,
        message = %req.message,
        "fleet alert received -- sending WhatsApp"
    );

    let action = format!("FLEET ALERT [{}] {}", req.severity.to_uppercase(), req.pod_id);
    whatsapp_alerter::send_admin_alert(&state.config, &action, &req.message).await;

    StatusCode::ACCEPTED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fleet_alert_request_deserializes() {
        let json = r#"{"pod_id":"pod-8","message":"test alert message","severity":"info"}"#;
        let req: FleetAlertRequest = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(req.pod_id, "pod-8");
        assert_eq!(req.message, "test alert message");
        assert_eq!(req.severity, "info");
    }

    #[test]
    fn test_fleet_alert_request_deserializes_critical() {
        let json = r#"{"pod_id":"pod-3","message":"3+ failed restarts","severity":"critical"}"#;
        let req: FleetAlertRequest = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(req.pod_id, "pod-3");
        assert_eq!(req.severity, "critical");
    }
}
