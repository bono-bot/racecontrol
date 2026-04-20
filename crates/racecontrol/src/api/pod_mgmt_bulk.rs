#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;
use crate::wol;
use rc_common::types::*;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};

// POST /pods/wake-all — Wake all pods with known MACs
pub(crate) async fn wake_all_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();
    let mut results = Vec::new();

    for pod in &pods {
        if let Some(mac) = &pod.mac_address {
            let status = match wol::send_wol(mac).await {
                Ok(_) => "sent",
                Err(_) => "failed",
            };
            results.push(json!({ "pod_id": pod.id, "mac": mac, "status": status }));
        }
    }

    Json(json!({ "status": "ok", "results": results }))
}

// POST /pods/shutdown-all — Shutdown all reachable pods
pub(crate) async fn shutdown_all_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();
    let service_key = state.config.pods.sentry_service_key.as_deref().unwrap_or("");
    let mut results = Vec::new();

    for pod in &pods {
        if pod.status == PodStatus::Offline || pod.status == PodStatus::Disabled {
            results.push(json!({ "pod_id": pod.id, "status": "skipped" }));
            continue;
        }
        let status = match wol::shutdown_pod(&state.http_client, &pod.ip_address, service_key).await {
            Ok(_) => {
                if let Some(p) = state.pods.write().await.get_mut(&pod.id) {
                    p.status = PodStatus::Disabled;
                    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(p.clone()));
                }
                "sent"
            }
            Err(_) => "failed",
        };
        results.push(json!({ "pod_id": pod.id, "status": status }));
    }

    Json(json!({ "status": "ok", "results": results }))
}

// POST /pods/restart-all — Restart all reachable pods
pub(crate) async fn restart_all_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();
    let service_key = state.config.pods.sentry_service_key.as_deref().unwrap_or("");
    let mut results = Vec::new();

    for pod in &pods {
        if pod.status == PodStatus::Offline || pod.status == PodStatus::Disabled {
            results.push(json!({ "pod_id": pod.id, "status": "skipped" }));
            continue;
        }
        let status = match wol::restart_pod(&state.http_client, &pod.ip_address, service_key).await {
            Ok(_) => "sent",
            Err(_) => "failed",
        };
        results.push(json!({ "pod_id": pod.id, "status": status }));
    }

    Json(json!({ "status": "ok", "results": results }))
}

// POST /pods/lockdown-all — Toggle kiosk lockdown for all connected pods
// Body: { "locked": true }
// Skips billing-active pods (when locking) and disconnected/closed senders.
pub(crate) async fn lockdown_all_pods(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let locked = body.get("locked")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // MMA-P2: Snapshot senders + billing state, drop locks before .await loop
    let send_targets: Vec<(String, _)> = {
        let active_timers = state.billing.active_timers.read().await;
        let senders = state.agent_senders.read().await;
        senders.iter()
            .filter(|(_, s)| !s.is_closed())
            .filter(|(pod_id, _)| !locked || !active_timers.contains_key(*pod_id))
            .map(|(pod_id, sender)| (pod_id.clone(), sender.clone()))
            .collect()
    }; // locks dropped here

    let mut results = Vec::new();
    // Collect skipped pods info
    {
        let active_timers = state.billing.active_timers.read().await;
        let senders = state.agent_senders.read().await;
        for (pod_id, sender) in senders.iter() {
            if sender.is_closed() {
                results.push(json!({ "pod_id": pod_id, "status": "not_connected" }));
            } else if locked && active_timers.contains_key(pod_id) {
                results.push(json!({ "pod_id": pod_id, "status": "skipped_billing_active" }));
            }
        }
    }

    for (pod_id, sender) in &send_targets {
        let mut settings = std::collections::HashMap::new();
        settings.insert(
            "kiosk_lockdown_enabled".to_string(),
            if locked { "true" } else { "false" }.to_string(),
        );
        let msg = CoreToAgentMessage::SettingsUpdated { settings };
        let _ = sender.send(CoreMessage::wrap(msg)).await;
        results.push(json!({ "pod_id": pod_id, "status": "sent" }));
    }

    Json(json!({ "ok": true, "locked": locked, "results": results }))
}

// POST /pods/{id}/unrestrict — Fully unrestrict a pod for employee training/maintenance.
// Sends ClearLockScreen + EnterDebugMode + disables kiosk lockdown on that pod.
// To re-restrict, use POST /pods/{id}/lockdown {"locked": true}.
// Body: { "unrestrict": true } (or false to re-restrict via lockdown + blank screen)
pub(crate) async fn unrestrict_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let unrestrict = body.get("unrestrict").and_then(|v| v.as_bool()).unwrap_or(true);

    // Clone sender, drop lock (standing rule: no lock across .await)
    let sender = {
        let senders = state.agent_senders.read().await;
        match senders.get(&id) {
            Some(s) if !s.is_closed() => s.clone(),
            _ => return Json(json!({ "error": format!("Pod {} not connected", id) })),
        }
    };

    if unrestrict {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::EnterDebugMode {
            employee_name: "Staff (admin panel)".to_string(),
        })).await;
        let mut settings = std::collections::HashMap::new();
        settings.insert("kiosk_lockdown_enabled".to_string(), "false".to_string());
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::SettingsUpdated { settings })).await;
        tracing::info!("Pod {} UNRESTRICTED for employee training", id);
    } else {
        let mut settings = std::collections::HashMap::new();
        settings.insert("kiosk_lockdown_enabled".to_string(), "true".to_string());
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::SettingsUpdated { settings })).await;
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::BlankScreen)).await;
        tracing::info!("Pod {} RESTRICTED — kiosk re-engaged", id);
    }

    // Optimistic update for dashboard
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(&id) {
            pod.screen_blanked = Some(!unrestrict);
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        }
    }

    Json(json!({ "ok": true, "pod_id": id, "unrestricted": unrestrict }))
}

// POST /pods/{id}/freedom — Toggle freedom mode on a pod.
// Freedom mode: all restrictions lifted (like unrestrict), but passive process monitoring stays active.
// Body: { "enabled": true } (or false to exit freedom mode and re-engage kiosk)
pub(crate) async fn freedom_mode_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let enabled = body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);

    // Clone sender, drop lock (standing rule: no lock across .await)
    let sender = {
        let senders = state.agent_senders.read().await;
        match senders.get(&id) {
            Some(s) if !s.is_closed() => s.clone(),
            _ => return Json(json!({ "error": format!("Pod {} not connected", id) })),
        }
    };

    if enabled {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::EnterFreedomMode)).await;
        tracing::info!("Pod {} FREEDOM MODE enabled — monitoring active", id);
    } else {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ExitFreedomMode)).await;
        tracing::info!("Pod {} FREEDOM MODE disabled — kiosk re-engaged", id);
    }

    // Optimistic update for dashboard
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(&id) {
            pod.freedom_mode = Some(enabled);
            pod.screen_blanked = Some(false);
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        }
    }

    Json(json!({ "ok": true, "pod_id": id, "freedom_mode": enabled }))
}
