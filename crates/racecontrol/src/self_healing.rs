//! v29.0 Phase 27: Self-healing orchestration — connects anomaly detection to recovery actions.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

const LOG_TARGET: &str = "self-heal-orch";

#[derive(Debug, Clone, Serialize)]
pub enum HealingAction {
    RestartPod(u8),
    RestartGameProcess(u8, String),
    ClearDiskSpace(u8),
    KillOrphanProcesses(u8),
    MarkPodDegraded(u8),
    MarkPodUnavailable(u8),
    EscalateToStaff(u8, String),
}

#[derive(Debug, Clone, Serialize)]
pub struct HealingOutcome {
    pub action: String,
    pub pod_id: u8,
    pub success: bool,
    pub before_value: Option<f64>,
    pub after_value: Option<f64>,
    pub timestamp: DateTime<Utc>,
    pub notes: String,
}

/// Map an anomaly alert to a healing action.
pub fn recommend_action(rule_name: &str, severity: &str, pod_id: u8) -> HealingAction {
    match (rule_name, severity) {
        ("Handle Leak", _) => HealingAction::KillOrphanProcesses(pod_id),
        ("Disk Space Critical", _) => HealingAction::ClearDiskSpace(pod_id),
        ("GPU Critical Temp", _) => HealingAction::MarkPodUnavailable(pod_id),
        ("GPU Overheat", _) => HealingAction::MarkPodDegraded(pod_id),
        ("Memory Pressure", "Critical") => HealingAction::RestartPod(pod_id),
        (_, "Critical") => HealingAction::EscalateToStaff(pod_id, rule_name.to_string()),
        (_, "High") => HealingAction::MarkPodDegraded(pod_id),
        _ => HealingAction::EscalateToStaff(pod_id, format!("{} ({})", rule_name, severity)),
    }
}

/// Pod availability state for kiosk/PWA/POS.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum PodAvailability {
    Available,
    Degraded { reason: String },
    MaintenanceHold { until: Option<String>, reason: String },
    Unavailable { reason: String },
}

/// Thread-safe map of pod_id -> availability state.
pub type PodAvailabilityMap = Arc<RwLock<HashMap<u8, PodAvailability>>>;

/// Create a new availability map with all 8 pods set to Available.
pub fn new_availability_map() -> PodAvailabilityMap {
    let mut map = HashMap::new();
    for pod_id in 1..=8 {
        map.insert(pod_id, PodAvailability::Available);
    }
    Arc::new(RwLock::new(map))
}

/// Update pod availability based on healing action.
pub async fn apply_action(map: &PodAvailabilityMap, action: &HealingAction) {
    let mut m = map.write().await;
    match action {
        HealingAction::MarkPodDegraded(id) => {
            m.insert(
                *id,
                PodAvailability::Degraded {
                    reason: "Anomaly detected".into(),
                },
            );
            tracing::warn!(target: LOG_TARGET, pod = id, "Pod marked DEGRADED");
        }
        HealingAction::MarkPodUnavailable(id) => {
            m.insert(
                *id,
                PodAvailability::Unavailable {
                    reason: "Critical anomaly".into(),
                },
            );
            tracing::error!(target: LOG_TARGET, pod = id, "Pod marked UNAVAILABLE");
        }
        _ => {}
    }
}

/// Clear pod state back to available (after successful recovery validation).
pub async fn mark_available(map: &PodAvailabilityMap, pod_id: u8) {
    let mut m = map.write().await;
    m.insert(pod_id, PodAvailability::Available);
    tracing::info!(target: LOG_TARGET, pod = pod_id, "Pod marked AVAILABLE");
}
