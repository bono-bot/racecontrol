#![allow(unused_imports)]
use rand::Rng;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::ac_server;
use crate::accounting;
use crate::fleet_alert;
use crate::recovery;
use crate::cafe;
use crate::config_push;
use crate::flags;
use crate::policy_engine;
use crate::preset_library;
use crate::cafe_alerts;
use crate::cafe_marketing;
use crate::cafe_promos;
use crate::auth;
use crate::whatsapp_alerter;
use crate::psychology;
use crate::auth::middleware::{require_staff_jwt, require_role_manager, require_role_superadmin};
use crate::network_source::require_non_pod_source;
use crate::billing;
use crate::catalog;
use crate::cloud_sync;
use crate::fleet_health;
use crate::fleet_intelligence;
use crate::process_guard;
use crate::friends;
use crate::game_launcher;
use crate::multiplayer;
use crate::pod_reservation;
use crate::reservation;
use crate::scheduler;
use crate::wallet;
use crate::weekend;
use crate::maintenance_store;
use crate::state::{AppState, VenueConfigSnapshot};
use crate::venue_shutdown;
use crate::wol;
use rc_common::pod_id::normalize_pod_id;
use rc_common::types::*;
use rc_common::protocol::{CloudAction, CoreMessage, CoreToAgentMessage, DashboardEvent};

pub(crate) async fn venue_info(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "name": state.config.venue.name,
        "location": state.config.venue.location,
        "timezone": state.config.venue.timezone,
        "pods": state.config.pods.count,
    }))
}

pub(crate) async fn list_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods = state.pods.read().await;
    let pod_list: Vec<&PodInfo> = pods.values().collect();
    Json(json!({ "pods": pod_list }))
}

pub(crate) async fn pod_status_summary(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods = state.pods.read().await;
    let total = pods.len();
    let mut down: Vec<Value> = Vec::new();
    for pod in pods.values() {
        if pod.status == PodStatus::Offline || pod.status == PodStatus::Error {
            down.push(json!({
                "id": pod.id,
                "number": pod.number,
                "status": pod.status,
            }));
        }
    }
    let active = total - down.len();
    Json(json!({
        "active": active,
        "total": total,
        "down": down,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

pub(crate) async fn get_pod(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    let pods = state.pods.read().await;
    match pods.get(&id) {
        Some(pod) => Json(json!({ "pod": pod })),
        None => Json(json!({ "error": "Pod not found" })),
    }
}

pub(crate) async fn register_pod(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = body["id"].as_str().unwrap_or("").to_string();
    let number = body["number"].as_u64().unwrap_or(0) as u32;
    let name = body["name"].as_str().unwrap_or("").to_string();
    let ip = body["ip_address"].as_str().unwrap_or("").to_string();
    let sim = body["sim_type"].as_str().unwrap_or("assetto_corsa");
    let sim_type = match sim {
        "assetto_corsa_evo" => SimType::AssettoCorsaEvo,
        "iracing" => SimType::IRacing,
        "f1_25" => SimType::F125,
        "le_mans_ultimate" | "lemans" => SimType::LeMansUltimate,
        "forza" => SimType::Forza,
        _ => SimType::AssettoCorsa,
    };

    let pod = PodInfo {
        id: id.clone(),
        number,
        name,
        ip_address: ip,
        mac_address: None,
        sim_type,
        status: PodStatus::Idle,
        current_driver: None,
        current_session_id: None,
        last_seen: Some(chrono::Utc::now()),
        driving_state: None,
        billing_session_id: None,
        game_state: None,
        current_game: None,
        installed_games: vec![],
        screen_blanked: None,
        ffb_preset: None,
        freedom_mode: None,
        agent_timestamp: None, // Intentional default: server-side pod creation has no agent clock
        recent_lap_times: std::collections::VecDeque::new(),
    };

    state.pods.write().await.insert(id.clone(), pod.clone());
    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));

    Json(json!({ "ok": true, "pod": pod }))
}

pub(crate) async fn seed_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    // (id, number, name, ip, mac)
    let pod_data = vec![
        ("pod_1", 1, "Pod 1", "192.168.31.89", "30:56:0F:05:45:88"),
        ("pod_2", 2, "Pod 2", "192.168.31.33", "30:56:0F:05:46:53"),
        ("pod_3", 3, "Pod 3", "192.168.31.28", "30:56:0F:05:44:B3"),
        ("pod_4", 4, "Pod 4", "192.168.31.88", "30:56:0F:05:45:25"),
        ("pod_5", 5, "Pod 5", "192.168.31.86", "30:56:0F:05:44:B7"),
        ("pod_6", 6, "Pod 6", "192.168.31.87", "30:56:0F:05:45:6E"),
        ("pod_7", 7, "Pod 7", "192.168.31.38", "30:56:0F:05:44:B4"),
        ("pod_8", 8, "Pod 8", "192.168.31.91", "30:56:0F:05:46:C5"),
    ];

    let mut pods_created = Vec::new();
    for (id, number, name, ip, mac) in pod_data {
        let pod = PodInfo {
            id: id.to_string(),
            number,
            name: name.to_string(),
            ip_address: ip.to_string(),
            mac_address: Some(mac.to_string()),
            sim_type: SimType::AssettoCorsa,
            status: PodStatus::Idle,
            current_driver: None,
            current_session_id: None,
            last_seen: Some(chrono::Utc::now()),
            driving_state: None,
            billing_session_id: None,
            game_state: None,
            current_game: None,
            installed_games: vec![],
            screen_blanked: None,
            ffb_preset: None,
            freedom_mode: None,
            agent_timestamp: None, // Intentional default: server-side pod seeding has no agent clock
            recent_lap_times: std::collections::VecDeque::new(),
        };
        state.pods.write().await.insert(id.to_string(), pod.clone());
        let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        pods_created.push(pod);
    }

    // Also send a full pod list event
    let all_pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();
    let _ = state.dashboard_tx.send(DashboardEvent::PodList(all_pods));

    Json(json!({ "ok": true, "count": pods_created.len() }))
}

// POST /pods/{id}/wake — Send Wake-on-LAN magic packet
pub(crate) async fn wake_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let pods = state.pods.read().await;
    let pod = match pods.get(&id) {
        Some(p) => p.clone(),
        None => return Json(json!({ "error": format!("Pod {} not found", id) })),
    };
    drop(pods);

    let mac = match &pod.mac_address {
        Some(m) => m.clone(),
        None => return Json(json!({ "error": format!("No MAC address for pod {}", id) })),
    };

    match wol::send_wol(&mac).await {
        Ok(_) => Json(json!({ "status": "wol_sent", "pod_id": id, "mac": mac })),
        Err(e) => Json(json!({ "error": format!("WoL failed: {}", e) })),
    }
}

// POST /pods/{id}/shutdown — Shutdown pod via pod-agent
pub(crate) async fn shutdown_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let pods = state.pods.read().await;
    let pod = match pods.get(&id) {
        Some(p) => p.clone(),
        None => return Json(json!({ "error": format!("Pod {} not found", id) })),
    };
    drop(pods);

    match wol::shutdown_pod(&state.http_client, &pod.ip_address).await {
        Ok(output) => {
            // Mark pod as Disabled — prevents auto-recovery from waking it back up
            if let Some(p) = state.pods.write().await.get_mut(&id) {
                p.status = PodStatus::Disabled;
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(p.clone()));
            }
            Json(json!({ "status": "shutdown_sent", "pod_id": id, "output": output }))
        }
        Err(e) => Json(json!({ "error": format!("Shutdown failed: {}", e) })),
    }
}

// POST /pods/{id}/enable — Re-enable a disabled pod (allows auto-recovery)
pub(crate) async fn enable_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let mut pods = state.pods.write().await;
    match pods.get_mut(&id) {
        Some(pod) => {
            pod.status = PodStatus::Offline;
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            drop(pods); // release write lock before log call
            // Phase 307 AUDIT-03: Log admin pod enable action
            crate::activity_log::log_pod_activity(
                &state, &id, "admin", "Pod Enabled", "", "staff", None,
            );
            Json(json!({ "status": "enabled", "pod_id": id }))
        }
        None => Json(json!({ "error": format!("Pod {} not found", id) })),
    }
}

// POST /pods/{id}/disable — Disable a pod (prevents all auto-recovery, no shutdown)
pub(crate) async fn disable_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let mut pods = state.pods.write().await;
    match pods.get_mut(&id) {
        Some(pod) => {
            pod.status = PodStatus::Disabled;
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            drop(pods); // release write lock before log call
            // Phase 307 AUDIT-03: Log admin pod disable action
            crate::activity_log::log_pod_activity(
                &state, &id, "admin", "Pod Disabled", "", "staff", None,
            );
            Json(json!({ "status": "disabled", "pod_id": id }))
        }
        None => Json(json!({ "error": format!("Pod {} not found", id) })),
    }
}

// POST /pods/:id/screen — Blank or unblank a specific pod's screen
pub(crate) async fn set_pod_screen(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let blank = body.get("blank").and_then(|v| v.as_bool()).unwrap_or(false);

    // MMA-P1: Clone sender out of read lock, drop guard BEFORE .await
    // Prevents deadlock/starvation when holding RwLock across async boundaries
    let sender = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&id).cloned()
    }; // read lock dropped here

    match sender {
        Some(sender) => {
            let msg = if blank {
                CoreToAgentMessage::BlankScreen
            } else {
                CoreToAgentMessage::ClearLockScreen
            };
            let _ = sender.send(CoreMessage::wrap(msg)).await;

            // Optimistic update: reflect blank state immediately so kiosk sees the change
            // without waiting for the next heartbeat cycle
            {
                let mut pods = state.pods.write().await;
                if let Some(pod) = pods.get_mut(&id) {
                    pod.screen_blanked = Some(blank);
                    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
                }
            }

            Json(json!({ "ok": true, "pod_id": id, "blank": blank }))
        }
        None => Json(json!({ "error": format!("Pod {} not connected", id) })),
    }
}

// POST /pods/{id}/restart — Restart pod via pod-agent (does NOT mark Disabled)
pub(crate) async fn restart_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let pods = state.pods.read().await;
    let pod = match pods.get(&id) {
        Some(p) => p.clone(),
        None => return Json(json!({ "error": format!("Pod {} not found", id) })),
    };
    drop(pods);

    match wol::restart_pod(&state.http_client, &pod.ip_address).await {
        Ok(output) => Json(json!({ "status": "restart_sent", "pod_id": id, "output": output })),
        Err(e) => Json(json!({ "error": format!("Restart failed: {}", e) })),
    }
}

// POST /pods/{id}/lockdown — Toggle kiosk lockdown for a specific pod
// Body: { "locked": true }
// Guard: rejects pods with active billing (if locking) and disconnected pods.
pub(crate) async fn lockdown_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let locked = body.get("locked")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Guard: do not lock pods with active billing
    if locked && state.billing.active_timers.read().await.contains_key(&id) {
        return Json(json!({ "error": "pod has active billing session" }));
    }

    // MMA-P2: Clone sender, drop lock before .await
    let sender = {
        let senders = state.agent_senders.read().await;
        match senders.get(&id) {
            Some(s) if !s.is_closed() => s.clone(),
            _ => return Json(json!({ "error": "pod not connected" })),
        }
    };

    let mut settings = std::collections::HashMap::new();
    settings.insert(
        "kiosk_lockdown_enabled".to_string(),
        if locked { "true" } else { "false" }.to_string(),
    );
    let msg = CoreToAgentMessage::SettingsUpdated { settings };
    let _ = sender.send(CoreMessage::wrap(msg)).await;

    // Phase 307 AUDIT-03: Log admin lockdown action for hash chain coverage
    let lockdown_action = if locked { "Pod Lockdown" } else { "Pod Lockdown Released" };
    crate::activity_log::log_pod_activity(
        &state,
        &id,
        "admin",
        lockdown_action,
        &format!("locked={}", locked),
        "staff",
        None,
    );

    Json(json!({ "ok": true, "pod_id": id, "locked": locked }))
}

// Bulk pod operations (wake_all, shutdown_all, restart_all, lockdown_all,
// unrestrict_pod, freedom_mode_pod) are in pod_mgmt_bulk.rs
