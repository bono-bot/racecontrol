use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::ac_server;
use crate::auth;
use crate::billing;
use crate::catalog;
use crate::friends;
use crate::game_launcher;
use crate::multiplayer;
use crate::pod_reservation;
use crate::scheduler;
use crate::wallet;
use crate::state::AppState;
use crate::wol;
use rc_common::types::*;
use rc_common::protocol::{CoreToAgentMessage, DashboardEvent};

pub fn api_routes() -> Router<Arc<AppState>> {
    Router::new()
        // Health
        .route("/health", get(health))
        // Pods
        .route("/pods", get(list_pods).post(register_pod))
        .route("/pods/seed", post(seed_pods))
        .route("/pods/{id}", get(get_pod))
        .route("/pods/{id}/wake", post(wake_pod))
        .route("/pods/{id}/shutdown", post(shutdown_pod))
        .route("/pods/{id}/enable", post(enable_pod))
        .route("/pods/{id}/disable", post(disable_pod))
        .route("/pods/{id}/screen", post(set_pod_screen))
        .route("/pods/wake-all", post(wake_all_pods))
        .route("/pods/shutdown-all", post(shutdown_all_pods))
        // Drivers
        .route("/drivers", get(list_drivers).post(create_driver))
        .route("/drivers/{id}", get(get_driver))
        // Sessions
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/sessions/{id}", get(get_session))
        // Laps
        .route("/laps", get(list_laps))
        .route("/sessions/{id}/laps", get(session_laps))
        // Leaderboard
        .route("/leaderboard/{track}", get(track_leaderboard))
        // Events
        .route("/events", get(list_events).post(create_event))
        // Bookings
        .route("/bookings", get(list_bookings).post(create_booking))
        // Pricing
        .route("/pricing", get(list_pricing_tiers).post(create_pricing_tier))
        .route("/pricing/{id}", put(update_pricing_tier).delete(delete_pricing_tier))
        // Billing
        .route("/billing/start", post(start_billing))
        .route("/billing/active", get(active_billing_sessions))
        .route("/billing/sessions", get(list_billing_sessions))
        .route("/billing/sessions/{id}", get(get_billing_session))
        .route("/billing/sessions/{id}/events", get(billing_session_events))
        .route("/billing/sessions/{id}/summary", get(billing_session_summary))
        .route("/billing/{id}/stop", post(stop_billing))
        .route("/billing/{id}/pause", post(pause_billing))
        .route("/billing/{id}/resume", post(resume_billing))
        .route("/billing/{id}/extend", post(extend_billing))
        .route("/billing/report/daily", get(daily_billing_report))
        // Game Launcher
        .route("/games/launch", post(launch_game))
        .route("/games/stop", post(stop_game))
        .route("/games/active", get(active_games))
        .route("/games/history", get(game_launch_history))
        .route("/games/pod/{pod_id}", get(pod_game_state))
        // AC LAN
        .route("/ac/presets", get(list_ac_presets).post(save_ac_preset))
        .route("/ac/presets/{id}", get(get_ac_preset).put(update_ac_preset).delete(delete_ac_preset))
        .route("/ac/session/start", post(start_ac_session))
        .route("/ac/session/stop", post(stop_ac_session))
        .route("/ac/session/active", get(active_ac_session))
        .route("/ac/sessions", get(list_ac_sessions))
        .route("/ac/content/tracks", get(list_ac_tracks))
        .route("/ac/content/cars", get(list_ac_cars))
        // Venue info
        .route("/venue", get(venue_info))
        // Auth (staff-facing)
        .route("/auth/assign", post(assign_customer))
        .route("/auth/cancel/{id}", post(cancel_assignment))
        .route("/auth/pending", get(pending_auth_tokens))
        .route("/auth/pending/{pod_id}", get(pending_auth_token_for_pod))
        // Auth (staff override — start billing without PIN/QR)
        .route("/auth/start-now", post(start_now))
        // Auth (agent-facing)
        .route("/auth/validate-pin", post(validate_pin))
        // Auth (kiosk-facing — no pod_id required)
        .route("/auth/kiosk/validate-pin", post(kiosk_validate_pin))
        // Auth (PWA-facing)
        .route("/auth/validate-qr", post(validate_qr))
        // Wallet (staff-facing)
        .route("/wallet/bonus-tiers", get(wallet_bonus_tiers))
        .route("/wallet/{driver_id}", get(get_wallet))
        .route("/wallet/{driver_id}/topup", post(topup_wallet))
        .route("/wallet/{driver_id}/transactions", get(wallet_transactions))
        .route("/wallet/{driver_id}/debit", post(debit_wallet_manual))
        .route("/wallet/{driver_id}/refund", post(refund_wallet))
        // Customer (PWA endpoints)
        .route("/customer/login", post(customer_login))
        .route("/customer/verify-otp", post(customer_verify_otp))
        .route("/customer/register", post(customer_register))
        .route("/customer/waiver-status", get(customer_waiver_status))
        .route("/customer/profile", get(customer_profile).put(customer_update_profile))
        .route("/customer/sessions", get(customer_sessions))
        .route("/customer/sessions/{id}", get(customer_session_detail))
        .route("/customer/laps", get(customer_laps))
        .route("/customer/stats", get(customer_stats))
        .route("/customer/wallet", get(customer_wallet))
        .route("/customer/wallet/transactions", get(customer_wallet_transactions))
        .route("/customer/experiences", get(customer_experiences))
        .route("/customer/ac/catalog", get(customer_ac_catalog))
        .route("/customer/book", post(customer_book_session))
        .route("/customer/active-reservation", get(customer_active_reservation))
        .route("/customer/end-reservation", post(customer_end_reservation))
        .route("/customer/continue-session", post(customer_continue_session))
        // Friends (PWA)
        .route("/customer/friends", get(customer_friends))
        .route("/customer/friends/requests", get(customer_friend_requests))
        .route("/customer/friends/request", post(customer_send_friend_request))
        .route("/customer/friends/request/{id}/accept", post(customer_accept_friend_request))
        .route("/customer/friends/request/{id}/reject", post(customer_reject_friend_request))
        .route("/customer/friends/{id}", axum::routing::delete(customer_remove_friend))
        .route("/customer/presence", put(customer_set_presence))
        // Multiplayer (PWA)
        .route("/customer/book-multiplayer", post(customer_book_multiplayer))
        .route("/customer/group-session", get(customer_group_session))
        .route("/customer/group-session/{id}/accept", post(customer_accept_group_invite))
        .route("/customer/group-session/{id}/decline", post(customer_decline_group_invite))
        // Telemetry (PWA)
        .route("/customer/telemetry", get(customer_telemetry))
        // Waivers (admin-facing)
        .route("/waivers", get(list_waivers))
        .route("/waivers/check", get(check_waiver))
        .route("/waivers/{driver_id}/signature", get(get_waiver_signature))
        // Kiosk
        .route("/kiosk/experiences", get(list_kiosk_experiences).post(create_kiosk_experience))
        .route("/kiosk/experiences/{id}", get(get_kiosk_experience).put(update_kiosk_experience).delete(delete_kiosk_experience))
        .route("/kiosk/settings", get(get_kiosk_settings).put(update_kiosk_settings))
        // AI Chat
        .route("/ai/chat", post(ai_chat))
        .route("/customer/ai/chat", post(customer_ai_chat))
        // AI Diagnose (on-demand analysis)
        .route("/ai/diagnose", post(ai_diagnose))
        // AI Suggestions (history)
        .route("/ai/suggestions", get(list_ai_suggestions))
        .route("/ai/suggestions/{id}/dismiss", post(dismiss_ai_suggestion))
        // AI Training Management
        .route("/ai/training/stats", get(ai_training_stats))
        .route("/ai/training/pairs", get(ai_training_pairs))
        .route("/ai/training/import", post(ai_training_import))
        // Cloud action queue
        .route("/actions", post(create_action))
        .route("/actions/pending", get(pending_actions))
        .route("/actions/{id}/ack", post(ack_action))
        .route("/actions/history", get(action_history))
        // Cloud sync
        .route("/sync/changes", get(sync_changes))
        .route("/sync/push", post(sync_push))
        .route("/sync/health", get(sync_health))
        // Terminal (remote command execution)
        .route("/terminal/auth", post(terminal_auth))
        .route("/terminal/commands", get(terminal_list).post(terminal_submit))
        .route("/terminal/commands/pending", get(terminal_pending))
        .route("/terminal/commands/{id}/result", post(terminal_result))
        // Staff
        .route("/staff/validate-pin", post(staff_validate_pin))
        .route("/staff", get(list_staff).post(create_staff))
        // Employee
        .route("/employee/daily-pin", get(employee_daily_pin))
        .route("/employee/debug-unlock", post(employee_debug_unlock))
        // Dynamic Pricing & Coupons (admin)
        .route("/pricing/rules", get(list_pricing_rules).post(create_pricing_rule))
        .route("/pricing/rules/{id}", put(update_pricing_rule).delete(delete_pricing_rule))
        .route("/coupons", get(list_coupons).post(create_coupon))
        .route("/coupons/{id}", put(update_coupon).delete(delete_coupon))
        // Review Nudges (admin)
        .route("/review-nudges/pending", get(pending_review_nudges))
        .route("/review-nudges/{id}/sent", post(mark_nudge_sent))
        // Time Trial Admin
        .route("/time-trials", get(list_time_trials).post(create_time_trial))
        .route("/time-trials/{id}", put(update_time_trial).delete(delete_time_trial))
        // Tournaments (admin + public)
        .route("/tournaments", get(list_tournaments).post(create_tournament))
        .route("/tournaments/{id}", get(get_tournament).put(update_tournament))
        .route("/tournaments/{id}/registrations", get(tournament_registrations))
        .route("/tournaments/{id}/matches", get(tournament_matches))
        .route("/tournaments/{id}/generate-bracket", post(generate_bracket))
        .route("/tournaments/{id}/matches/{match_id}/result", post(record_match_result))
        // Tournament (PWA customer)
        .route("/customer/tournaments", get(customer_list_tournaments))
        .route("/customer/tournaments/{id}/register", post(customer_register_tournament))
        // Coaching / Telemetry comparison (PWA)
        .route("/customer/compare-laps", get(customer_compare_laps))
        // Smart Scheduler
        .route("/scheduler/status", get(scheduler::get_status))
        .route("/scheduler/settings", put(scheduler::update_settings))
        .route("/scheduler/analytics", get(scheduler::get_analytics))
        // Session share report (PWA)
        .route("/customer/sessions/{id}/share", get(customer_session_share))
        // Referrals (PWA)
        .route("/customer/referral-code", get(customer_referral_code))
        .route("/customer/referral-code/generate", post(customer_generate_referral_code))
        .route("/customer/redeem-referral", post(customer_redeem_referral))
        // Coupons (PWA)
        .route("/customer/apply-coupon", post(customer_apply_coupon))
        // Packages (PWA)
        .route("/customer/packages", get(customer_list_packages))
        // Memberships (PWA)
        .route("/customer/membership", get(customer_membership))
        .route("/customer/membership/subscribe", post(customer_subscribe_membership))
        // Public (no auth)
        .route("/public/leaderboard", get(public_leaderboard))
        .route("/public/leaderboard/{track}", get(public_track_leaderboard))
        .route("/public/time-trial", get(public_time_trial))
        // Bot (WhatsApp bot — terminal_secret auth)
        .route("/bot/lookup", get(bot_lookup))
        .route("/bot/pricing", get(bot_pricing))
        .route("/bot/book", post(bot_book))
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "racecontrol",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn venue_info(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "name": state.config.venue.name,
        "location": state.config.venue.location,
        "timezone": state.config.venue.timezone,
        "pods": state.config.pods.count,
    }))
}

async fn list_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods = state.pods.read().await;
    let pod_list: Vec<&PodInfo> = pods.values().collect();
    Json(json!({ "pods": pod_list }))
}

async fn get_pod(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    let pods = state.pods.read().await;
    match pods.get(&id) {
        Some(pod) => Json(json!({ "pod": pod })),
        None => Json(json!({ "error": "Pod not found" })),
    }
}

async fn register_pod(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = body["id"].as_str().unwrap_or("").to_string();
    let number = body["number"].as_u64().unwrap_or(0) as u32;
    let name = body["name"].as_str().unwrap_or("").to_string();
    let ip = body["ip_address"].as_str().unwrap_or("").to_string();
    let sim = body["sim_type"].as_str().unwrap_or("assetto_corsa");
    let sim_type = match sim {
        "iracing" => SimType::IRacing,
        "f1_25" => SimType::F125,
        "lemans" => SimType::LeMansUltimate,
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
    };

    state.pods.write().await.insert(id.clone(), pod.clone());
    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));

    Json(json!({ "ok": true, "pod": pod }))
}

async fn seed_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
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
async fn wake_pod(
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
async fn shutdown_pod(
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
async fn enable_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let mut pods = state.pods.write().await;
    match pods.get_mut(&id) {
        Some(pod) => {
            pod.status = PodStatus::Offline;
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            Json(json!({ "status": "enabled", "pod_id": id }))
        }
        None => Json(json!({ "error": format!("Pod {} not found", id) })),
    }
}

// POST /pods/{id}/disable — Disable a pod (prevents all auto-recovery, no shutdown)
async fn disable_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let mut pods = state.pods.write().await;
    match pods.get_mut(&id) {
        Some(pod) => {
            pod.status = PodStatus::Disabled;
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            Json(json!({ "status": "disabled", "pod_id": id }))
        }
        None => Json(json!({ "error": format!("Pod {} not found", id) })),
    }
}

// POST /pods/:id/screen — Blank or unblank a specific pod's screen
async fn set_pod_screen(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let blank = body.get("blank").and_then(|v| v.as_bool()).unwrap_or(false);

    let agent_senders = state.agent_senders.read().await;
    match agent_senders.get(&id) {
        Some(sender) => {
            let msg = if blank {
                CoreToAgentMessage::BlankScreen
            } else {
                CoreToAgentMessage::ClearLockScreen
            };
            let _ = sender.send(msg).await;
            Json(json!({ "ok": true, "pod_id": id, "blank": blank }))
        }
        None => Json(json!({ "error": format!("Pod {} not connected", id) })),
    }
}

// POST /pods/wake-all — Wake all pods with known MACs
async fn wake_all_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
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
async fn shutdown_all_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();
    let mut results = Vec::new();

    for pod in &pods {
        if pod.status == PodStatus::Offline || pod.status == PodStatus::Disabled {
            results.push(json!({ "pod_id": pod.id, "status": "skipped" }));
            continue;
        }
        let status = match wol::shutdown_pod(&state.http_client, &pod.ip_address).await {
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

#[derive(Debug, Deserialize)]
struct ListDriversQuery {
    search: Option<String>,
}

async fn list_drivers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListDriversQuery>,
) -> Json<Value> {
    let rows = if let Some(ref search) = params.search {
        let q = format!("%{}%", search);
        sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64, Option<String>)>(
            "SELECT id, name, email, phone, total_laps, total_time_ms, customer_id
             FROM drivers
             WHERE name LIKE ?1 COLLATE NOCASE OR phone LIKE ?2
             ORDER BY name LIMIT 20",
        )
        .bind(&q)
        .bind(&q)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64, Option<String>)>(
            "SELECT id, name, email, phone, total_laps, total_time_ms, customer_id
             FROM drivers ORDER BY name",
        )
        .fetch_all(&state.db)
        .await
    };

    match rows {
        Ok(drivers) => {
            let list: Vec<Value> = drivers.iter().map(|d| json!({
                "id": d.0, "name": d.1, "email": d.2, "phone": d.3,
                "total_laps": d.4, "total_time_ms": d.5, "customer_id": d.6,
            })).collect();
            Json(json!({ "drivers": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_driver(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
    let email = body.get("email").and_then(|v| v.as_str());
    let phone = body.get("phone").and_then(|v| v.as_str());
    let steam_guid = body.get("steam_guid").and_then(|v| v.as_str());

    let result = sqlx::query(
        "INSERT INTO drivers (id, name, email, phone, steam_guid, updated_at) VALUES (?, ?, ?, ?, ?, datetime('now'))"
    )
    .bind(&id)
    .bind(name)
    .bind(email)
    .bind(phone)
    .bind(steam_guid)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "name": name })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_driver(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64)>(
        "SELECT id, name, email, phone, total_laps, total_time_ms FROM drivers WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(d)) => Json(json!({
            "id": d.0, "name": d.1, "email": d.2, "phone": d.3,
            "total_laps": d.4, "total_time_ms": d.5,
        })),
        Ok(None) => Json(json!({ "error": "Driver not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>)>(
        "SELECT id, type, sim_type, track, status, started_at FROM sessions ORDER BY created_at DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions.iter().map(|s| json!({
                "id": s.0, "type": s.1, "sim_type": s.2,
                "track": s.3, "status": s.4, "started_at": s.5,
            })).collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let session_type = body.get("type").and_then(|v| v.as_str()).unwrap_or("practice");
    let sim_type = body.get("sim_type").and_then(|v| v.as_str()).unwrap_or("assetto_corsa");
    let track = body.get("track").and_then(|v| v.as_str()).unwrap_or("monza");
    let car_class = body.get("car_class").and_then(|v| v.as_str());

    let result = sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track, car_class, status) VALUES (?, ?, ?, ?, ?, 'pending')"
    )
    .bind(&id)
    .bind(session_type)
    .bind(sim_type)
    .bind(track)
    .bind(car_class)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "type": session_type, "track": track })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_session(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    use sqlx::Row;

    let row = sqlx::query(
        "SELECT bs.id, bs.driver_id, d.name as driver_name, bs.pod_id,
                bs.pricing_tier_id, pt.name as tier_name,
                bs.allocated_seconds, bs.driving_seconds, bs.status,
                COALESCE(bs.custom_price_paise, pt.price_paise) as price_paise,
                bs.started_at, bs.ended_at,
                bs.experience_id, ke.name as experience_name,
                bs.car, bs.track, bs.sim_type,
                bs.reservation_id, bs.wallet_txn_id,
                bs.wallet_debit_paise, bs.created_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         LEFT JOIN kiosk_experiences ke ON bs.experience_id = ke.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Get laps count and best lap for this session
    let lap_stats = sqlx::query_as::<_, (i64, Option<i64>)>(
        "SELECT COUNT(*), MIN(CASE WHEN valid = 1 THEN lap_time_ms END)
         FROM laps WHERE session_id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or((0, None));

    Json(json!({
        "session": {
            "id": row.get::<String, _>("id"),
            "driver_id": row.get::<String, _>("driver_id"),
            "driver_name": row.get::<String, _>("driver_name"),
            "pod_id": row.get::<String, _>("pod_id"),
            "pricing_tier_id": row.get::<String, _>("pricing_tier_id"),
            "pricing_tier_name": row.get::<String, _>("tier_name"),
            "allocated_seconds": row.get::<i64, _>("allocated_seconds"),
            "driving_seconds": row.get::<i64, _>("driving_seconds"),
            "status": row.get::<String, _>("status"),
            "price_paise": row.get::<i64, _>("price_paise"),
            "started_at": row.get::<Option<String>, _>("started_at"),
            "ended_at": row.get::<Option<String>, _>("ended_at"),
            "experience_id": row.get::<Option<String>, _>("experience_id"),
            "experience_name": row.get::<Option<String>, _>("experience_name"),
            "car": row.get::<Option<String>, _>("car"),
            "track": row.get::<Option<String>, _>("track"),
            "sim_type": row.get::<Option<String>, _>("sim_type"),
            "reservation_id": row.get::<Option<String>, _>("reservation_id"),
            "wallet_txn_id": row.get::<Option<String>, _>("wallet_txn_id"),
            "wallet_debit_paise": row.get::<Option<i64>, _>("wallet_debit_paise"),
            "created_at": row.get::<String, _>("created_at"),
            "total_laps": lap_stats.0,
            "best_lap_ms": lap_stats.1,
        }
    }))
}

async fn list_laps(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, i64, Option<i64>, Option<i64>, Option<i64>, bool)>(
        "SELECT id, driver_id, track, car, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid FROM laps ORDER BY created_at DESC LIMIT 100"
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(laps) => {
            let list: Vec<Value> = laps.iter().map(|l| json!({
                "id": l.0, "driver_id": l.1, "track": l.2, "car": l.3,
                "lap_time_ms": l.4, "sector1_ms": l.5, "sector2_ms": l.6,
                "sector3_ms": l.7, "valid": l.8,
            })).collect();
            Json(json!({ "laps": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn session_laps(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (
        String, String, String, String, String, i64, i64,
        Option<i64>, Option<i64>, Option<i64>, bool, String,
    )>(
        "SELECT l.id, l.driver_id, l.pod_id, l.track, l.car, l.lap_number, l.lap_time_ms,
                l.sector1_ms, l.sector2_ms, l.sector3_ms, l.valid, l.created_at
         FROM laps l
         WHERE l.session_id = ?
         ORDER BY l.lap_number ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(laps) => {
            let list: Vec<Value> = laps
                .iter()
                .map(|l| {
                    json!({
                        "id": l.0,
                        "driver_id": l.1,
                        "pod_id": l.2,
                        "track": l.3,
                        "car": l.4,
                        "lap_number": l.5,
                        "lap_time_ms": l.6,
                        "sector1_ms": l.7,
                        "sector2_ms": l.8,
                        "sector3_ms": l.9,
                        "valid": l.10,
                        "created_at": l.11,
                    })
                })
                .collect();
            let count = list.len();
            Json(json!({ "session_id": id, "laps": list, "count": count }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn track_leaderboard(State(state): State<Arc<AppState>>, Path(track): Path<String>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, i64, String)>(
        "SELECT tr.track, tr.car, CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END, tr.best_lap_ms, tr.achieved_at
         FROM track_records tr JOIN drivers d ON tr.driver_id = d.id
         WHERE tr.track = ? ORDER BY tr.best_lap_ms ASC"
    )
    .bind(&track)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(records) => {
            let list: Vec<Value> = records.iter().enumerate().map(|(i, r)| json!({
                "position": i + 1,
                "track": r.0, "car": r.1, "driver": r.2,
                "best_lap_ms": r.3, "achieved_at": r.4,
            })).collect();
            Json(json!({ "track": track, "records": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn list_events(State(_state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "events": [] }))
}

async fn create_event(State(_state): State<Arc<AppState>>, Json(_body): Json<Value>) -> Json<Value> {
    Json(json!({ "todo": "create_event" }))
}

async fn list_bookings(State(_state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "bookings": [] }))
}

async fn create_booking(State(_state): State<Arc<AppState>>, Json(_body): Json<Value>) -> Json<Value> {
    Json(json!({ "todo": "create_booking" }))
}

// ─── Pricing ────────────────────────────────────────────────────────────────

async fn list_pricing_tiers(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, i64, i64, bool, bool, i64)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial, is_active, sort_order
         FROM pricing_tiers ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(tiers) => {
            let list: Vec<Value> = tiers
                .iter()
                .map(|t| {
                    json!({
                        "id": t.0, "name": t.1, "duration_minutes": t.2,
                        "price_paise": t.3, "is_trial": t.4, "is_active": t.5,
                        "sort_order": t.6,
                    })
                })
                .collect();
            Json(json!({ "tiers": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_pricing_tier(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("Custom");
    let duration_minutes = body.get("duration_minutes").and_then(|v| v.as_i64()).unwrap_or(30);
    let price_paise = body.get("price_paise").and_then(|v| v.as_i64()).unwrap_or(0);
    let is_trial = body.get("is_trial").and_then(|v| v.as_bool()).unwrap_or(false);
    let sort_order = body.get("sort_order").and_then(|v| v.as_i64()).unwrap_or(10);

    let result = sqlx::query(
        "INSERT INTO pricing_tiers (id, name, duration_minutes, price_paise, is_trial, sort_order)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(duration_minutes)
    .bind(price_paise)
    .bind(is_trial)
    .bind(sort_order)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "name": name })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_pricing_tier(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let name = body.get("name").and_then(|v| v.as_str());
    let duration_minutes = body.get("duration_minutes").and_then(|v| v.as_i64());
    let price_paise = body.get("price_paise").and_then(|v| v.as_i64());
    let is_active = body.get("is_active").and_then(|v| v.as_bool());

    // Build dynamic update query
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(n) = name {
        updates.push("name = ?");
        binds.push(n.to_string());
    }
    if let Some(d) = duration_minutes {
        updates.push("duration_minutes = ?");
        binds.push(d.to_string());
    }
    if let Some(p) = price_paise {
        updates.push("price_paise = ?");
        binds.push(p.to_string());
    }
    if let Some(a) = is_active {
        updates.push("is_active = ?");
        binds.push(if a { "1".to_string() } else { "0".to_string() });
    }

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" }));
    }

    let query = format!("UPDATE pricing_tiers SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_pricing_tier(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Soft delete: set is_active = 0
    match sqlx::query("UPDATE pricing_tiers SET is_active = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Billing ────────────────────────────────────────────────────────────────

async fn start_billing(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id = body.get("pod_id").and_then(|v| v.as_str()).unwrap_or("");
    let driver_id = body.get("driver_id").and_then(|v| v.as_str()).unwrap_or("");
    let pricing_tier_id = body.get("pricing_tier_id").and_then(|v| v.as_str()).unwrap_or("");
    let custom_price_paise = body.get("custom_price_paise").and_then(|v| v.as_u64()).map(|v| v as u32);
    let custom_duration_minutes = body.get("custom_duration_minutes").and_then(|v| v.as_u64()).map(|v| v as u32);
    let staff_id = body.get("staff_id").and_then(|v| v.as_str()).map(|s| s.to_string());

    if pod_id.is_empty() || driver_id.is_empty() || pricing_tier_id.is_empty() {
        return Json(json!({ "error": "pod_id, driver_id, and pricing_tier_id are required" }));
    }

    // Pre-validate to return useful errors instead of silent failures
    {
        let timers = state.billing.active_timers.read().await;
        if timers.contains_key(pod_id) {
            return Json(json!({ "error": format!("Pod {} already has an active billing session", pod_id) }));
        }
    }

    let tier_exists = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if tier_exists.is_none() {
        return Json(json!({ "error": format!("Pricing tier '{}' not found or inactive", pricing_tier_id) }));
    }

    let driver_exists = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if driver_exists.is_none() {
        return Json(json!({ "error": format!("Driver '{}' not found", driver_id) }));
    }

    // Look up tier price to determine wallet debit amount
    let tier_info = sqlx::query_as::<_, (i64, bool)>(
        "SELECT price_paise, is_trial FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (tier_price_paise, is_trial) = match tier_info {
        Some(t) => (t.0, t.1),
        None => return Json(json!({ "error": "Pricing tier lookup failed" })),
    };

    // Determine the actual price (custom override or tier price)
    let price_paise = custom_price_paise.map(|p| p as i64).unwrap_or(tier_price_paise);

    // Wallet balance check and debit (skip for trial or zero-price)
    let wallet_debit: Option<i64> = if !is_trial && price_paise > 0 {
        // Check balance first
        let balance = match wallet::get_balance(&state, driver_id).await {
            Ok(b) => b,
            Err(e) => return Json(json!({ "error": format!("Wallet error: {}", e) })),
        };
        if balance < price_paise {
            return Json(json!({
                "error": format!("Insufficient credits: have {} credits, need {} credits", balance / 100, price_paise / 100),
                "balance_paise": balance,
                "required_paise": price_paise,
            }));
        }

        // Debit wallet
        let pod_num = sqlx::query_as::<_, (i64,)>("SELECT number FROM pods WHERE id = ?")
            .bind(pod_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .map(|r| r.0)
            .unwrap_or(0);

        match wallet::debit(
            &state,
            driver_id,
            price_paise,
            "debit_session",
            None,
            Some(&format!("Session on Pod {} (staff)", pod_num)),
        )
        .await
        {
            Ok((_, _txn_id)) => Some(price_paise),
            Err(e) => return Json(json!({ "error": e })),
        }
    } else {
        None
    };

    // Now start billing (should succeed since we pre-validated)
    let session_id = billing::start_billing_session(
        &state,
        pod_id.to_string(),
        driver_id.to_string(),
        pricing_tier_id.to_string(),
        custom_price_paise,
        custom_duration_minutes,
        staff_id,
    )
    .await;

    match session_id {
        Ok(id) => {
            // Record wallet debit on the billing session
            if let Some(debit) = wallet_debit {
                let _ = sqlx::query(
                    "UPDATE billing_sessions SET wallet_debit_paise = ? WHERE id = ?",
                )
                .bind(debit)
                .bind(&id)
                .execute(&state.db)
                .await;
            }
            Json(json!({ "ok": true, "billing_session_id": id, "wallet_debit_paise": wallet_debit }))
        }
        Err(reason) => {
            // Refund wallet if billing failed
            if let Some(debit) = wallet_debit {
                let _ = wallet::refund(&state, driver_id, debit, None, Some("Billing start failed — auto-refund")).await;
            }
            state.record_api_error("billing/start");
            Json(json!({ "error": reason }))
        }
    }
}

async fn stop_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let cmd = rc_common::protocol::DashboardCommand::EndBilling {
        billing_session_id: id,
    };
    billing::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

async fn pause_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let cmd = rc_common::protocol::DashboardCommand::PauseBilling {
        billing_session_id: id,
    };
    billing::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

async fn resume_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let cmd = rc_common::protocol::DashboardCommand::ResumeBilling {
        billing_session_id: id,
    };
    billing::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

async fn extend_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let additional_seconds = body
        .get("additional_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(600) as u32;

    let cmd = rc_common::protocol::DashboardCommand::ExtendBilling {
        billing_session_id: id,
        additional_seconds,
    };
    billing::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

async fn active_billing_sessions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let timers = state.billing.active_timers.read().await;
    let sessions: Vec<_> = timers.values().map(|t| t.to_info()).collect();
    Json(json!({ "sessions": sessions }))
}

#[derive(Deserialize)]
struct BillingListQuery {
    date: Option<String>,
    status: Option<String>,
}

async fn list_billing_sessions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<BillingListQuery>,
) -> Json<Value> {
    let mut query = String::from(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds,
                bs.driving_seconds, bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at, bs.created_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE 1=1",
    );

    if let Some(date) = &params.date {
        query.push_str(&format!(" AND date(bs.started_at) = '{}'", date));
    }
    if let Some(status) = &params.status {
        query.push_str(&format!(" AND bs.status = '{}'", status));
    }

    query.push_str(" ORDER BY bs.created_at DESC LIMIT 100");

    let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, i64, Option<String>, Option<String>, String)>(
        &query,
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0, "driver_id": s.1, "driver_name": s.2,
                        "pod_id": s.3, "pricing_tier_name": s.4,
                        "allocated_seconds": s.5, "driving_seconds": s.6,
                        "status": s.7, "price_paise": s.8,
                        "started_at": s.9, "ended_at": s.10, "created_at": s.11,
                    })
                })
                .collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_billing_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, i64, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds,
                bs.driving_seconds, bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(s)) => Json(json!({
            "id": s.0, "driver_id": s.1, "driver_name": s.2,
            "pod_id": s.3, "pricing_tier_name": s.4,
            "allocated_seconds": s.5, "driving_seconds": s.6,
            "status": s.7, "price_paise": s.8,
            "started_at": s.9, "ended_at": s.10,
        })),
        Ok(None) => Json(json!({ "error": "Billing session not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn billing_session_events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, i64, Option<String>, String)>(
        "SELECT id, event_type, driving_seconds_at_event, metadata, created_at
         FROM billing_events WHERE billing_session_id = ? ORDER BY created_at ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(events) => {
            let list: Vec<Value> = events
                .iter()
                .map(|e| {
                    json!({
                        "id": e.0, "event_type": e.1,
                        "driving_seconds_at_event": e.2,
                        "metadata": e.3, "created_at": e.4,
                    })
                })
                .collect();
            Json(json!({ "events": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn billing_session_summary(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Get billing session info
    let session = sqlx::query_as::<_, (String, String, String, String, i64, i64, String, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, bs.allocated_seconds, bs.driving_seconds, bs.status, bs.started_at, bs.ended_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Get laps for this session
    let laps = sqlx::query_as::<_, (String, i64, i64, Option<i64>, Option<i64>, Option<i64>, bool, String, String)>(
        "SELECT id, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, track, car
         FROM laps WHERE session_id = ? ORDER BY lap_number ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let total_laps = laps.len() as u32;
    let valid_laps: Vec<_> = laps.iter().filter(|l| l.6).collect();
    let best_lap_ms = valid_laps.iter().map(|l| l.2).min();
    let avg_lap_ms = if !valid_laps.is_empty() {
        Some(valid_laps.iter().map(|l| l.2).sum::<i64>() / valid_laps.len() as i64)
    } else {
        None
    };

    // Check personal best
    let track = laps.first().map(|l| l.7.as_str()).unwrap_or("");
    let car = laps.first().map(|l| l.8.as_str()).unwrap_or("");

    let pb = sqlx::query_as::<_, (i64,)>(
        "SELECT best_lap_ms FROM personal_bests WHERE driver_id = ? AND track = ? AND car = ?",
    )
    .bind(&session.1)
    .bind(track)
    .bind(car)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let personal_best_broken = best_lap_ms.map(|b| pb.map(|p| b <= p.0).unwrap_or(true)).unwrap_or(false);

    // Check leaderboard position
    let leaderboard_position = if !track.is_empty() && !car.is_empty() {
        sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) + 1 FROM personal_bests WHERE track = ? AND car = ? AND best_lap_ms < ?",
        )
        .bind(track)
        .bind(car)
        .bind(best_lap_ms.unwrap_or(i64::MAX))
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0)
    } else {
        None
    };

    let laps_json: Vec<Value> = laps
        .iter()
        .map(|l| {
            json!({
                "lap_number": l.1,
                "lap_time_ms": l.2,
                "sector1_ms": l.3,
                "sector2_ms": l.4,
                "sector3_ms": l.5,
                "valid": l.6,
            })
        })
        .collect();

    Json(json!({
        "summary": {
            "billing_session_id": session.0,
            "driver_id": session.1,
            "driver_name": session.2,
            "pod_id": session.3,
            "track": track,
            "car": car,
            "allocated_seconds": session.4,
            "driving_seconds": session.5,
            "status": session.6,
            "total_laps": total_laps,
            "best_lap_ms": best_lap_ms,
            "average_lap_ms": avg_lap_ms,
            "personal_best_broken": personal_best_broken,
            "leaderboard_position": leaderboard_position,
            "laps": laps_json,
        }
    }))
}

#[derive(Deserialize)]
struct DailyReportQuery {
    date: Option<String>,
}

async fn daily_billing_report(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DailyReportQuery>,
) -> Json<Value> {
    let date = params
        .date
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

    let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, i64, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds,
                bs.driving_seconds, bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at, bs.staff_id, sm.name
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         LEFT JOIN staff_members sm ON bs.staff_id = sm.id
         WHERE date(bs.started_at) = ?
         ORDER BY bs.started_at ASC",
    )
    .bind(&date)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let total_sessions = sessions.len();
            let total_revenue_paise: i64 = sessions
                .iter()
                .filter(|s| s.7 != "cancelled")
                .map(|s| s.8)
                .sum();
            let total_driving_seconds: i64 = sessions.iter().map(|s| s.6).sum();

            // Build staff summary
            let mut staff_map: std::collections::HashMap<String, (String, usize, i64)> = std::collections::HashMap::new();
            for s in &sessions {
                if s.7 == "cancelled" { continue; }
                let staff_key = s.11.clone().unwrap_or_default();
                let staff_name = s.12.clone().unwrap_or_else(|| "Walk-in / Self".to_string());
                let entry = staff_map.entry(staff_key).or_insert((staff_name, 0, 0));
                entry.1 += 1;
                entry.2 += s.8;
            }
            let staff_summary: Vec<Value> = staff_map
                .into_iter()
                .map(|(id, (name, count, revenue))| {
                    json!({ "staff_id": id, "staff_name": name, "sessions": count, "revenue_paise": revenue })
                })
                .collect();

            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0, "driver_id": s.1, "driver_name": s.2,
                        "pod_id": s.3, "pricing_tier_name": s.4,
                        "allocated_seconds": s.5, "driving_seconds": s.6,
                        "status": s.7, "price_paise": s.8,
                        "started_at": s.9, "ended_at": s.10,
                        "staff_id": s.11, "staff_name": s.12,
                    })
                })
                .collect();

            Json(json!({
                "date": date,
                "total_sessions": total_sessions,
                "total_revenue_paise": total_revenue_paise,
                "total_driving_seconds": total_driving_seconds,
                "staff_summary": staff_summary,
                "sessions": list,
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Game Launcher ─────────────────────────────────────────────────────────

async fn launch_game(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id = body.get("pod_id").and_then(|v| v.as_str()).unwrap_or("");
    let sim_type_str = body.get("sim_type").and_then(|v| v.as_str()).unwrap_or("");
    let launch_args_raw = body
        .get("launch_args")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if pod_id.is_empty() || sim_type_str.is_empty() {
        return Json(json!({ "error": "pod_id and sim_type are required" }));
    }

    // Inject duration_minutes from active billing session into launch_args
    let launch_args = if let Some(args) = launch_args_raw {
        let duration_minutes: u32 = sqlx::query_as::<_, (i64,)>(
            "SELECT allocated_seconds FROM billing_sessions WHERE pod_id = ? AND status = 'active' ORDER BY started_at DESC LIMIT 1",
        )
        .bind(pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|(secs,)| (secs as u32) / 60)
        .unwrap_or(60);

        let mut parsed: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
        parsed["duration_minutes"] = serde_json::json!(duration_minutes);
        Some(parsed.to_string())
    } else {
        None
    };

    let sim_type: SimType = match serde_json::from_value(serde_json::Value::String(
        sim_type_str.to_string(),
    )) {
        Ok(st) => st,
        Err(_) => return Json(json!({ "error": format!("Unknown sim_type: {}", sim_type_str) })),
    };

    let cmd = rc_common::protocol::DashboardCommand::LaunchGame {
        pod_id: pod_id.to_string(),
        sim_type,
        launch_args,
    };

    game_launcher::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

async fn stop_game(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id = body.get("pod_id").and_then(|v| v.as_str()).unwrap_or("");

    if pod_id.is_empty() {
        return Json(json!({ "error": "pod_id is required" }));
    }

    let cmd = rc_common::protocol::DashboardCommand::StopGame {
        pod_id: pod_id.to_string(),
    };

    game_launcher::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

async fn active_games(State(state): State<Arc<AppState>>) -> Json<Value> {
    let games = state.game_launcher.active_games.read().await;
    let list: Vec<_> = games.values().map(|g| g.to_info()).collect();
    Json(json!({ "games": list }))
}

#[derive(Deserialize)]
struct GameHistoryQuery {
    pod_id: Option<String>,
    limit: Option<i64>,
}

async fn game_launch_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GameHistoryQuery>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(100).min(1000).max(1);

    let rows = if let Some(pod_id) = &params.pod_id {
        sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<String>, String)>(
            "SELECT id, pod_id, sim_type, event_type, pid, error_message, created_at \
             FROM game_launch_events WHERE pod_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(pod_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<String>, String)>(
            "SELECT id, pod_id, sim_type, event_type, pid, error_message, created_at \
             FROM game_launch_events ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
    };

    match rows {
        Ok(events) => {
            let list: Vec<Value> = events
                .iter()
                .map(|e| {
                    json!({
                        "id": e.0, "pod_id": e.1, "sim_type": e.2,
                        "event_type": e.3, "pid": e.4,
                        "error_message": e.5, "created_at": e.6,
                    })
                })
                .collect();
            Json(json!({ "events": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn pod_game_state(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    let games = state.game_launcher.active_games.read().await;
    match games.get(&pod_id) {
        Some(tracker) => Json(json!({ "game": tracker.to_info() })),
        None => Json(json!({ "game": null, "state": "idle" })),
    }
}

// ─── AC LAN ──────────────────────────────────────────────────────────────────

async fn list_ac_presets(State(state): State<Arc<AppState>>) -> Json<Value> {
    match ac_server::list_presets(&state).await {
        Ok(presets) => Json(json!({ "presets": presets })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn save_ac_preset(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => return Json(json!({ "error": "name is required" })),
    };

    let config: AcLanSessionConfig = match body.get("config") {
        Some(c) => match serde_json::from_value(c.clone()) {
            Ok(cfg) => cfg,
            Err(e) => return Json(json!({ "error": format!("Invalid config: {}", e) })),
        },
        None => return Json(json!({ "error": "config is required" })),
    };

    match ac_server::save_preset(&state, &name, &config).await {
        Ok(id) => Json(json!({ "id": id, "name": name })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_ac_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match ac_server::load_preset(&state, &id).await {
        Ok((name, config)) => Json(json!({ "id": id, "name": name, "config": config })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_ac_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let name = body.get("name").and_then(|v| v.as_str());
    let config = body.get("config").and_then(|c| serde_json::from_value::<AcLanSessionConfig>(c.clone()).ok());

    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(n) = name {
        updates.push("name = ?");
        binds.push(n.to_string());
    }
    if let Some(cfg) = &config {
        updates.push("config_json = ?");
        binds.push(serde_json::to_string(cfg).unwrap_or_default());
    }

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" }));
    }

    updates.push("updated_at = datetime('now')");
    let query = format!("UPDATE ac_presets SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(r) if r.rows_affected() == 0 => Json(json!({ "error": "Preset not found" })),
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_ac_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match ac_server::delete_preset(&state, &id).await {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn start_ac_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let config: AcLanSessionConfig = match body.get("config") {
        Some(c) => match serde_json::from_value(c.clone()) {
            Ok(cfg) => cfg,
            Err(e) => return Json(json!({ "error": format!("Invalid config: {}", e) })),
        },
        None => return Json(json!({ "error": "config is required" })),
    };

    let pod_ids: Vec<String> = body
        .get("pod_ids")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    match ac_server::start_ac_server(&state, config, pod_ids).await {
        Ok(session_id) => Json(json!({ "session_id": session_id })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn stop_ac_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let session_id = match body.get("session_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Json(json!({ "error": "session_id is required" })),
    };

    match ac_server::stop_ac_server(&state, session_id).await {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn active_ac_session(State(state): State<Arc<AppState>>) -> Json<Value> {
    let instances = state.ac_server.instances.read().await;
    let active: Vec<_> = instances
        .values()
        .filter(|i| matches!(i.status, AcServerStatus::Running | AcServerStatus::Starting))
        .map(|i| i.to_info())
        .collect();
    Json(json!({ "sessions": active }))
}

#[derive(Deserialize)]
struct AcSessionsQuery {
    status: Option<String>,
    limit: Option<i64>,
}

async fn list_ac_sessions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AcSessionsQuery>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(50).min(1000).max(1);

    let rows = if let Some(status) = &params.status {
        sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, String)>(
            "SELECT id, preset_id, status, pod_ids, pid, join_url, error_message, started_at, ended_at, created_at \
             FROM ac_sessions WHERE status = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(status)
        .bind(limit)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, String)>(
            "SELECT id, preset_id, status, pod_ids, pid, join_url, error_message, started_at, ended_at, created_at \
             FROM ac_sessions ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
    };

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0, "preset_id": s.1, "status": s.2,
                        "pod_ids": s.3, "pid": s.4, "join_url": s.5,
                        "error_message": s.6, "started_at": s.7,
                        "ended_at": s.8, "created_at": s.9,
                    })
                })
                .collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn list_ac_tracks(State(_state): State<Arc<AppState>>) -> Json<Value> {
    // Curated list of popular AC tracks
    Json(json!({ "tracks": [
        { "id": "monza", "name": "Monza", "configs": ["", "junior"] },
        { "id": "spa", "name": "Spa-Francorchamps", "configs": [""] },
        { "id": "silverstone", "name": "Silverstone", "configs": ["", "international", "national", "gp"] },
        { "id": "brands_hatch", "name": "Brands Hatch", "configs": ["", "gp", "indy"] },
        { "id": "nurburgring", "name": "Nurburgring", "configs": ["", "sprint"] },
        { "id": "nordschleife", "name": "Nordschleife", "configs": ["", "endurance", "tourist"] },
        { "id": "mugello", "name": "Mugello", "configs": [""] },
        { "id": "imola", "name": "Imola", "configs": [""] },
        { "id": "barcelona", "name": "Barcelona", "configs": ["", "moto", "national"] },
        { "id": "ks_red_bull_ring", "name": "Red Bull Ring", "configs": ["", "national"] },
        { "id": "vallelunga", "name": "Vallelunga", "configs": ["", "club"] },
        { "id": "drift", "name": "Drift Track", "configs": [""] },
        { "id": "ks_zandvoort", "name": "Zandvoort", "configs": [""] },
        { "id": "ks_laguna_seca", "name": "Laguna Seca", "configs": [""] },
        { "id": "suzuka", "name": "Suzuka", "configs": ["", "east"] },
        { "id": "ks_highlands", "name": "Highlands", "configs": [""] },
        { "id": "ks_black_cat_county", "name": "Black Cat County", "configs": ["", "long"] },
        { "id": "magione", "name": "Magione", "configs": [""] },
        { "id": "trento-bondone", "name": "Trento Bondone", "configs": [""] },
    ]}))
}

async fn list_ac_cars(State(_state): State<Arc<AppState>>) -> Json<Value> {
    // Curated list of popular AC cars grouped by class
    Json(json!({ "cars": [
        { "id": "ks_ferrari_488_gt3", "name": "Ferrari 488 GT3", "class": "GT3" },
        { "id": "ks_lamborghini_huracan_gt3", "name": "Lamborghini Huracan GT3", "class": "GT3" },
        { "id": "ks_mercedes_amg_gt3", "name": "Mercedes AMG GT3", "class": "GT3" },
        { "id": "ks_audi_r8_lms_2016", "name": "Audi R8 LMS 2016", "class": "GT3" },
        { "id": "ks_porsche_911_gt3_r_2016", "name": "Porsche 911 GT3 R", "class": "GT3" },
        { "id": "ks_mclaren_650_gt3", "name": "McLaren 650S GT3", "class": "GT3" },
        { "id": "ks_nissan_gtr_gt3", "name": "Nissan GT-R GT3", "class": "GT3" },
        { "id": "ks_bmw_m6_gt3", "name": "BMW M6 GT3", "class": "GT3" },
        { "id": "ks_ferrari_488_gtb", "name": "Ferrari 488 GTB", "class": "Street" },
        { "id": "ks_lamborghini_huracan_performante", "name": "Lamborghini Huracan Performante", "class": "Street" },
        { "id": "ks_porsche_911_r", "name": "Porsche 911 R", "class": "Street" },
        { "id": "ks_mclaren_p1", "name": "McLaren P1", "class": "Hypercar" },
        { "id": "ks_ferrari_laferrari", "name": "Ferrari LaFerrari", "class": "Hypercar" },
        { "id": "ks_porsche_918_spyder", "name": "Porsche 918 Spyder", "class": "Hypercar" },
        { "id": "ks_audi_r18_etron_quattro", "name": "Audi R18 e-tron", "class": "LMP" },
        { "id": "ks_porsche_919_hybrid_2016", "name": "Porsche 919 Hybrid", "class": "LMP" },
        { "id": "ks_toyota_ts040", "name": "Toyota TS040", "class": "LMP" },
        { "id": "tatuusfa1", "name": "Tatuus FA01", "class": "Open Wheel" },
        { "id": "ks_ferrari_sf15t", "name": "Ferrari SF15-T", "class": "Open Wheel" },
        { "id": "lotus_exos_125_s1", "name": "Lotus Exos 125 S1", "class": "Open Wheel" },
        { "id": "ks_mazda_mx5_cup", "name": "Mazda MX-5 Cup", "class": "Cup" },
        { "id": "ks_toyota_gt86", "name": "Toyota GT86", "class": "Street" },
        { "id": "ks_ford_mustang_2015", "name": "Ford Mustang 2015", "class": "Street" },
        { "id": "ks_abarth_595ss_s2", "name": "Abarth 595 SS", "class": "Street" },
        { "id": "lotus_2_eleven", "name": "Lotus 2-Eleven", "class": "Track Day" },
        { "id": "ks_toyota_ae86_drift", "name": "Toyota AE86 Drift", "class": "Drift" },
        { "id": "ks_nissan_370z", "name": "Nissan 370Z", "class": "Drift" },
    ]}))
}

// ─── Auth Endpoints ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AssignCustomerRequest {
    pod_id: String,
    driver_id: String,
    pricing_tier_id: String,
    auth_type: String,
    custom_price_paise: Option<u32>,
    custom_duration_minutes: Option<u32>,
    experience_id: Option<String>,
}

async fn assign_customer(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AssignCustomerRequest>,
) -> Json<Value> {
    match auth::create_auth_token(
        &state,
        req.pod_id,
        req.driver_id,
        req.pricing_tier_id,
        req.auth_type,
        req.custom_price_paise,
        req.custom_duration_minutes,
        req.experience_id,
        None, // custom_launch_args (staff assign doesn't use custom booking)
    )
    .await
    {
        Ok(token_info) => Json(json!({ "token": token_info })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn cancel_assignment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match auth::cancel_auth_token(&state, id).await {
        Ok(()) => Json(json!({ "status": "cancelled" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn pending_auth_tokens(State(state): State<Arc<AppState>>) -> Json<Value> {
    let tokens = auth::get_pending_tokens(&state).await;
    Json(json!({ "tokens": tokens }))
}

async fn pending_auth_token_for_pod(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    let tokens = auth::get_pending_tokens(&state).await;
    let token = tokens.into_iter().find(|t| t.pod_id == pod_id);
    match token {
        Some(t) => Json(json!({ "token": t })),
        None => Json(json!({ "token": null })),
    }
}

#[derive(Debug, Deserialize)]
struct ValidatePinRequest {
    pod_id: String,
    pin: String,
}

async fn validate_pin(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ValidatePinRequest>,
) -> Json<Value> {
    match auth::validate_pin(&state, req.pod_id, req.pin).await {
        Ok(billing_session_id) => Json(json!({
            "status": "ok",
            "billing_session_id": billing_session_id,
        })),
        Err(e) => {
            state.record_api_error("auth/validate-pin");
            Json(json!({ "error": e }))
        }
    }
}

#[derive(Debug, Deserialize)]
struct KioskValidatePinRequest {
    pin: String,
}

async fn kiosk_validate_pin(
    State(state): State<Arc<AppState>>,
    Json(req): Json<KioskValidatePinRequest>,
) -> Json<Value> {
    match auth::validate_pin_kiosk(&state, req.pin).await {
        Ok(result) => Json(json!({
            "status": "ok",
            "billing_session_id": result.billing_session_id,
            "pod_id": result.pod_id,
            "pod_number": result.pod_number,
            "driver_name": result.driver_name,
            "pricing_tier_name": result.pricing_tier_name,
            "allocated_seconds": result.allocated_seconds,
        })),
        Err(e) => {
            state.record_api_error("auth/kiosk-validate-pin");
            Json(json!({ "error": e }))
        }
    }
}

#[derive(Debug, Deserialize)]
struct StartNowRequest {
    token_id: String,
}

async fn start_now(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartNowRequest>,
) -> Json<Value> {
    match auth::start_now(&state, req.token_id).await {
        Ok(billing_session_id) => Json(json!({
            "status": "ok",
            "billing_session_id": billing_session_id,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Debug, Deserialize)]
struct ValidateQrRequest {
    qr_token: String,
    driver_id: String,
}

async fn validate_qr(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ValidateQrRequest>,
) -> Json<Value> {
    match auth::validate_qr(&state, req.qr_token, req.driver_id).await {
        Ok(billing_session_id) => Json(json!({
            "status": "ok",
            "billing_session_id": billing_session_id,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Customer PWA Endpoints ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CustomerLoginRequest {
    phone: String,
}

async fn customer_login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CustomerLoginRequest>,
) -> Json<Value> {
    match auth::send_otp(&state, &req.phone).await {
        Ok(_driver_id) => Json(json!({ "status": "otp_sent" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Debug, Deserialize)]
struct VerifyOtpRequest {
    phone: String,
    otp: String,
}

async fn customer_verify_otp(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VerifyOtpRequest>,
) -> Json<Value> {
    match auth::verify_otp(&state, &req.phone, &req.otp).await {
        Ok(jwt) => {
            // Check registration status
            let registered = sqlx::query_as::<_, (bool,)>(
                "SELECT COALESCE(registration_completed, 0) FROM drivers WHERE phone = ?",
            )
            .bind(&req.phone)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .map(|r| r.0)
            .unwrap_or(false);

            Json(json!({
                "status": "ok",
                "token": jwt,
                "registration_completed": registered,
            }))
        }
        Err(e) => Json(json!({ "error": e })),
    }
}

/// Extract driver_id from Authorization: Bearer <jwt> header
fn extract_driver_id(state: &AppState, headers: &axum::http::HeaderMap) -> Result<String, String> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "Missing Authorization header".to_string())?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| "Invalid Authorization format".to_string())?;

    auth::verify_jwt(token, &state.config.auth.jwt_secret)
}

async fn customer_profile(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let driver = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64, bool, Option<String>, Option<String>, bool)>(
        "SELECT id, name, email, phone, total_laps, total_time_ms, COALESCE(has_used_trial, 0), customer_id, nickname, COALESCE(show_nickname_on_leaderboard, 0) FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match driver {
        Ok(Some(d)) => {
            let wallet_balance = wallet::get_balance(&state, &d.0).await.unwrap_or(0);
            let active_reservation = pod_reservation::get_active_reservation_for_driver(&state, &d.0).await;

            Json(json!({
                "driver": {
                    "id": d.0,
                    "customer_id": d.7,
                    "name": d.1,
                    "nickname": d.8,
                    "show_nickname_on_leaderboard": d.9,
                    "email": d.2,
                    "phone": d.3,
                    "total_laps": d.4,
                    "total_time_ms": d.5,
                    "has_used_trial": d.6,
                    "wallet_balance_paise": wallet_balance,
                    "active_reservation": active_reservation,
                }
            }))
        }
        Ok(None) => Json(json!({ "error": "Driver not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn customer_update_profile(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    if let Some(nickname) = body.get("nickname") {
        let nick = nickname.as_str().map(|s| s.trim()).unwrap_or("");
        let nick_val: Option<&str> = if nick.is_empty() { None } else { Some(nick) };
        let _ = sqlx::query("UPDATE drivers SET nickname = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(nick_val)
            .bind(&driver_id)
            .execute(&state.db)
            .await;
    }

    if let Some(show) = body.get("show_nickname_on_leaderboard") {
        let val = show.as_bool().unwrap_or(false);
        let _ = sqlx::query("UPDATE drivers SET show_nickname_on_leaderboard = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(val)
            .bind(&driver_id)
            .execute(&state.db)
            .await;
    }

    Json(json!({ "status": "updated" }))
}

async fn customer_sessions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, i64, i64, String, Option<String>, Option<String>, Option<i64>)>(
        "SELECT bs.id, bs.pod_id, bs.allocated_seconds, bs.driving_seconds, bs.status, bs.started_at, bs.ended_at, bs.custom_price_paise
         FROM billing_sessions bs
         WHERE bs.driver_id = ?
         ORDER BY bs.created_at DESC
         LIMIT 50",
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0,
                        "pod_id": s.1,
                        "allocated_seconds": s.2,
                        "driving_seconds": s.3,
                        "status": s.4,
                        "started_at": s.5,
                        "ended_at": s.6,
                        "custom_price_paise": s.7,
                    })
                })
                .collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn customer_session_detail(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Fetch the billing session, ensuring it belongs to this customer
    let row = sqlx::query_as::<_, (
        String, String, String, i64, i64, String, i64,
        Option<String>, Option<String>,
        Option<String>, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<i64>,
    )>(
        "SELECT bs.id, bs.pod_id, pt.name, bs.allocated_seconds, bs.driving_seconds,
                bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at,
                bs.experience_id, ke.name,
                bs.car, bs.track, bs.sim_type,
                bs.wallet_debit_paise
         FROM billing_sessions bs
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         LEFT JOIN kiosk_experiences ke ON bs.experience_id = ke.id
         WHERE bs.id = ? AND bs.driver_id = ?",
    )
    .bind(&id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    let session = match row {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Look up any refund for this session
    let refund_paise: Option<(i64,)> = sqlx::query_as(
        "SELECT COALESCE(SUM(amount_paise), 0) FROM wallet_transactions
         WHERE reference_id = ? AND txn_type = 'refund'",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Get all laps for this session
    let laps = sqlx::query_as::<_, (
        String, i64, i64, Option<i64>, Option<i64>, Option<i64>, bool, String, String, String,
    )>(
        "SELECT id, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms,
                valid, track, car, created_at
         FROM laps WHERE session_id = ? AND driver_id = ?
         ORDER BY lap_number ASC",
    )
    .bind(&id)
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let total_laps = laps.len() as i64;
    let valid_laps: Vec<_> = laps.iter().filter(|l| l.6).collect();
    let best_lap_ms = valid_laps.iter().map(|l| l.2).min();
    let avg_lap_ms = if !valid_laps.is_empty() {
        Some(valid_laps.iter().map(|l| l.2).sum::<i64>() / valid_laps.len() as i64)
    } else {
        None
    };

    let laps_json: Vec<Value> = laps
        .iter()
        .map(|l| {
            json!({
                "id": l.0,
                "lap_number": l.1,
                "lap_time_ms": l.2,
                "sector1_ms": l.3,
                "sector2_ms": l.4,
                "sector3_ms": l.5,
                "valid": l.6,
                "track": l.7,
                "car": l.8,
                "created_at": l.9,
            })
        })
        .collect();

    Json(json!({
        "session": {
            "id": session.0,
            "pod_id": session.1,
            "pricing_tier_name": session.2,
            "allocated_seconds": session.3,
            "driving_seconds": session.4,
            "status": session.5,
            "price_paise": session.6,
            "started_at": session.7,
            "ended_at": session.8,
            "experience_id": session.9,
            "experience_name": session.10,
            "car": session.11,
            "track": session.12,
            "sim_type": session.13,
            "wallet_debit_paise": session.14,
            "refund_paise": refund_paise.map(|r| r.0).filter(|&r| r > 0),
            "total_laps": total_laps,
            "best_lap_ms": best_lap_ms,
            "average_lap_ms": avg_lap_ms,
        },
        "laps": laps_json,
    }))
}

async fn customer_laps(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, String, String, i64, Option<i64>, Option<i64>, Option<i64>, bool, String)>(
        "SELECT id, track, car, sim_type, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, created_at
         FROM laps
         WHERE driver_id = ?
         ORDER BY created_at DESC
         LIMIT 100",
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(laps) => {
            let list: Vec<Value> = laps
                .iter()
                .map(|l| {
                    json!({
                        "id": l.0,
                        "track": l.1,
                        "car": l.2,
                        "sim_type": l.3,
                        "lap_time_ms": l.4,
                        "sector1_ms": l.5,
                        "sector2_ms": l.6,
                        "sector3_ms": l.7,
                        "valid": l.8,
                        "created_at": l.9,
                    })
                })
                .collect();
            Json(json!({ "laps": list }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn customer_stats(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Total laps and time
    let totals = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COALESCE(total_laps, 0), COALESCE(total_time_ms, 0) FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or((0, 0));

    // Total sessions
    let session_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM billing_sessions WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    // Total driving time (seconds)
    let total_driving_secs = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(SUM(driving_seconds), 0) FROM billing_sessions WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    // Favourite car (most laps)
    let fav_car = sqlx::query_as::<_, (String, i64)>(
        "SELECT car, COUNT(*) as cnt FROM laps WHERE driver_id = ? GROUP BY car ORDER BY cnt DESC LIMIT 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Personal bests count
    let pb_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM personal_bests WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    Json(json!({
        "stats": {
            "total_laps": totals.0,
            "total_time_ms": totals.1,
            "total_sessions": session_count,
            "total_driving_seconds": total_driving_secs,
            "favourite_car": fav_car.as_ref().map(|c| &c.0),
            "personal_bests": pb_count,
        }
    }))
}

// ─── Customer Registration ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CustomerRegisterRequest {
    name: String,
    nickname: Option<String>,
    email: Option<String>,
    dob: String,
    waiver_consent: bool,
    signature_data: Option<String>,
    guardian_name: Option<String>,
    guardian_phone: Option<String>,
}

async fn customer_register(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CustomerRegisterRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    if !req.waiver_consent {
        return Json(json!({ "error": "Waiver consent is required" }));
    }

    let name = req.name.trim().to_string();
    if name.len() < 2 {
        return Json(json!({ "error": "Name must be at least 2 characters" }));
    }

    // Parse and validate DOB
    let dob = match chrono::NaiveDate::parse_from_str(&req.dob, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return Json(json!({ "error": "Invalid date format. Use YYYY-MM-DD" })),
    };

    let today = chrono::Utc::now().date_naive();
    let age = (today - dob).num_days() / 365;

    if age < 12 {
        return Json(json!({ "error": "Minimum age is 12 years" }));
    }

    // Guardian required for minors (12-17)
    if age < 18 {
        if req.guardian_name.as_ref().map_or(true, |n| n.trim().is_empty()) {
            return Json(json!({ "error": "Guardian name required for customers under 18" }));
        }
    }

    // Check for duplicate name + DOB (same person already registered)
    let duplicate: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM drivers WHERE name = ? AND dob = ? AND registration_completed = 1 AND id != ?",
    )
    .bind(&name)
    .bind(&req.dob)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    if duplicate.is_some() {
        return Json(json!({ "error": "An account with this name and date of birth already exists. Please sign in with your registered phone number." }));
    }

    let nickname = req.nickname.as_ref().map(|n| n.trim().to_string()).filter(|n| !n.is_empty());

    // Update driver record
    let result = sqlx::query(
        "UPDATE drivers SET
            name = ?, nickname = ?, email = ?, dob = ?,
            waiver_signed = 1, waiver_signed_at = datetime('now'),
            waiver_version = 'v1.0',
            signature_data = ?,
            guardian_name = ?, guardian_phone = ?,
            registration_completed = 1,
            updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(&name)
    .bind(&nickname)
    .bind(&req.email)
    .bind(&req.dob)
    .bind(&req.signature_data)
    .bind(&req.guardian_name)
    .bind(&req.guardian_phone)
    .bind(&driver_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            // Auto-create wallet for new customer
            let _ = wallet::ensure_wallet(&state, &driver_id).await;

            tracing::info!("Customer {} registered (age: {}, minor: {})", driver_id, age, age < 18);
            Json(json!({
                "status": "registered",
                "driver_id": driver_id,
                "is_minor": age < 18,
            }))
        }
        Err(e) => Json(json!({ "error": format!("Registration failed: {}", e) })),
    }
}

async fn customer_waiver_status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let row = sqlx::query_as::<_, (bool, bool)>(
        "SELECT COALESCE(waiver_signed, 0), COALESCE(registration_completed, 0) FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some((waiver, registered))) => Json(json!({
            "waiver_signed": waiver,
            "registration_completed": registered,
        })),
        Ok(None) => Json(json!({ "error": "Driver not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── Waivers (admin-facing) ──────────────────────────────────────────────────

async fn list_waivers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let page: i64 = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1).max(1);
    let per_page: i64 = params.get("per_page").and_then(|p| p.parse().ok()).unwrap_or(50).min(200).max(1);
    let offset = (page - 1) * per_page;

    let total = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM drivers WHERE waiver_signed = 1",
    )
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, name, phone, email, dob, waiver_signed_at, waiver_version, guardian_name, guardian_phone, signature_data
         FROM drivers WHERE waiver_signed = 1
         ORDER BY waiver_signed_at DESC
         LIMIT ? OFFSET ?",
    )
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(waivers) => {
            let list: Vec<Value> = waivers.iter().map(|w| {
                let is_minor = w.4.as_ref().map_or(false, |dob| {
                    chrono::NaiveDate::parse_from_str(dob, "%Y-%m-%d")
                        .map(|d| (chrono::Utc::now().date_naive() - d).num_days() / 365 < 18)
                        .unwrap_or(false)
                });
                json!({
                    "driver_id": w.0,
                    "name": w.1,
                    "phone": w.2,
                    "email": w.3,
                    "dob": w.4,
                    "waiver_signed_at": w.5,
                    "waiver_version": w.6,
                    "guardian_name": w.7,
                    "guardian_phone": w.8,
                    "has_signature": w.9.is_some(),
                    "is_minor": is_minor,
                })
            }).collect();
            Json(json!({ "waivers": list, "total": total, "page": page, "per_page": per_page }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn check_waiver(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let phone = params.get("phone");
    let email = params.get("email");

    if phone.is_none() && email.is_none() {
        return Json(json!({ "error": "Provide phone or email parameter" }));
    }

    let row = if let Some(phone) = phone {
        // Normalize: strip non-digits, use last 10
        let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
        let last10 = if digits.len() >= 10 { &digits[digits.len() - 10..] } else { &digits };
        sqlx::query_as::<_, (String, String, Option<String>, bool)>(
            "SELECT id, name, phone, COALESCE(waiver_signed, 0) FROM drivers WHERE phone LIKE '%' || ?",
        )
        .bind(last10)
        .fetch_optional(&state.db)
        .await
    } else if let Some(email) = email {
        sqlx::query_as::<_, (String, String, Option<String>, bool)>(
            "SELECT id, name, phone, COALESCE(waiver_signed, 0) FROM drivers WHERE LOWER(email) = LOWER(?)",
        )
        .bind(email)
        .fetch_optional(&state.db)
        .await
    } else {
        return Json(json!({ "error": "Provide phone or email parameter" }));
    };

    match row {
        Ok(Some((id, name, phone, signed))) => Json(json!({
            "signed": signed,
            "driver": { "id": id, "name": name, "phone": phone },
        })),
        Ok(None) => Json(json!({ "signed": false, "driver": null })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn get_waiver_signature(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT signature_data FROM drivers WHERE id = ? AND waiver_signed = 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some((Some(sig),))) => Json(json!({ "signature_data": sig })),
        Ok(Some((None,))) => Json(json!({ "error": "No signature on file" })),
        Ok(None) => Json(json!({ "error": "Waiver not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── AI Chat ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AiChatRequest {
    message: String,
    #[serde(default)]
    history: Vec<Value>,
}

/// Staff/admin AI chat — full business context.
async fn ai_chat(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AiChatRequest>,
) -> Json<Value> {
    if !state.config.ai_debugger.enabled || !state.config.ai_debugger.chat_enabled {
        return Json(json!({ "error": "AI chat is not enabled" }));
    }

    // Gather live business context
    let context = crate::ai::gather_business_context(
        &state.db,
        &state.pods,
        &state.billing,
        &state.game_launcher,
    )
    .await;

    let system_prompt = crate::ai::build_staff_prompt(&context);

    // Build messages array: system + history + new message
    let mut messages: Vec<Value> = vec![json!({
        "role": "system",
        "content": system_prompt,
    })];

    for msg in &req.history {
        messages.push(msg.clone());
    }

    messages.push(json!({
        "role": "user",
        "content": req.message,
    }));

    match crate::ai::query_ai(&state.config.ai_debugger, &messages, Some(&state.db), Some("staff_chat")).await {
        Ok((reply, model)) => Json(json!({
            "reply": reply,
            "model": model,
        })),
        Err(e) => Json(json!({
            "error": format!("AI query failed: {}", e),
        })),
    }
}

/// Customer AI chat — scoped to their own data only.
async fn customer_ai_chat(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<AiChatRequest>,
) -> Json<Value> {
    if !state.config.ai_debugger.enabled || !state.config.ai_debugger.chat_enabled {
        return Json(json!({ "error": "AI chat is not enabled" }));
    }

    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Gather customer-scoped context
    let context = crate::ai::gather_customer_context(&state.db, &driver_id).await;
    let system_prompt = crate::ai::build_customer_prompt(&context);

    let mut messages: Vec<Value> = vec![json!({
        "role": "system",
        "content": system_prompt,
    })];

    for msg in &req.history {
        messages.push(msg.clone());
    }

    messages.push(json!({
        "role": "user",
        "content": req.message,
    }));

    match crate::ai::query_ai(&state.config.ai_debugger, &messages, Some(&state.db), Some("customer_chat")).await {
        Ok((reply, model)) => Json(json!({
            "reply": reply,
            "model": model,
        })),
        Err(e) => Json(json!({
            "error": format!("AI query failed: {}", e),
        })),
    }
}

// ─── AI Diagnose (on-demand) ────────────────────────────────────────────────

/// Staff-triggered on-demand AI analysis of recent operational errors.
async fn ai_diagnose(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    if !state.config.ai_debugger.enabled {
        return Json(json!({ "error": "AI debugger is not enabled" }));
    }

    let db = &state.db;
    let mut context_parts: Vec<String> = Vec::new();

    // Recent crashes (last 10 minutes)
    let crashes = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT pod_id, sim_type, error_message, created_at FROM game_launch_events \
         WHERE event_type = 'crash' AND created_at > datetime('now', '-10 minutes') \
         ORDER BY created_at DESC LIMIT 10",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !crashes.is_empty() {
        let mut s = format!("RECENT CRASHES ({} in last 10 min):\n", crashes.len());
        for (pod, sim, err, time) in &crashes {
            s.push_str(&format!(
                "  - {} on pod {} at {} ({})\n",
                sim, pod, time,
                err.as_deref().unwrap_or("no details")
            ));
        }
        context_parts.push(s);
    }

    // Billing anomalies
    let stuck = sqlx::query_as::<_, (String, String)>(
        "SELECT pod_id, created_at FROM billing_sessions \
         WHERE status = 'pending' AND created_at < datetime('now', '-60 seconds') \
         AND created_at > datetime('now', '-10 minutes')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !stuck.is_empty() {
        context_parts.push(format!(
            "STUCK BILLING: {} session(s) stuck in 'pending' state",
            stuck.len()
        ));
    }

    let stale = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM billing_sessions \
         WHERE status = 'active' \
         AND datetime(started_at, '+' || allocated_seconds || ' seconds') < datetime('now', '-30 seconds')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    if stale > 0 {
        context_parts.push(format!(
            "STALE BILLING: {} session(s) still 'active' past allocated time",
            stale
        ));
    }

    // API error counts
    let api_errors = state.drain_api_error_counts();
    let high_errors: Vec<_> = api_errors.iter().filter(|(_, v)| **v >= 2).collect();
    if !high_errors.is_empty() {
        let mut s = String::from("API ERRORS (recent):\n");
        for (endpoint, count) in &high_errors {
            s.push_str(&format!("  {} — {} errors\n", endpoint, count));
        }
        context_parts.push(s);
    }

    // Pod connectivity
    let pods = state.pods.read().await;
    let connected = pods.len();
    let expected = state.config.pods.count as usize;
    if connected < expected {
        context_parts.push(format!(
            "POD CONNECTIVITY: {}/{} pods connected",
            connected, expected
        ));
    }
    drop(pods);

    if context_parts.is_empty() {
        return Json(json!({
            "status": "healthy",
            "message": "No operational issues detected in the last 10 minutes"
        }));
    }

    // Gather additional business context
    let biz_context = crate::ai::gather_business_context(
        &state.db,
        &state.pods,
        &state.billing,
        &state.game_launcher,
    )
    .await;

    let full_context = format!(
        "OPERATIONAL ISSUES:\n{}\n\nVENUE STATE:\n{}",
        context_parts.join("\n\n"),
        biz_context
    );

    let messages = vec![
        json!({
            "role": "system",
            "content": "You are James, AI operations assistant for RacingPoint eSports. \
                        Analyze the operational issues below alongside the current venue state. \
                        Provide root cause analysis, severity assessment, and specific actionable steps. \
                        Be concise but thorough."
        }),
        json!({
            "role": "user",
            "content": full_context
        }),
    ];

    match crate::ai::query_ai(&state.config.ai_debugger, &messages, Some(&state.db), Some("debug")).await {
        Ok((suggestion, model)) => {
            // Persist to ai_suggestions table
            let id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO ai_suggestions (id, pod_id, sim_type, error_context, suggestion, model, source) \
                 VALUES (?, 'venue', 'diagnostic', ?, ?, ?, 'diagnose')"
            )
            .bind(&id)
            .bind(&context_parts.join("\n"))
            .bind(&suggestion)
            .bind(&model)
            .execute(db)
            .await;

            Json(json!({
                "status": "analyzed",
                "issues_found": context_parts.len(),
                "suggestion": suggestion,
                "model": model,
                "suggestion_id": id,
            }))
        }
        Err(e) => Json(json!({
            "status": "error",
            "issues_found": context_parts.len(),
            "issues": context_parts,
            "error": format!("AI analysis failed: {}", e),
        })),
    }
}

// ─── AI Suggestions History ─────────────────────────────────────────────────

async fn list_ai_suggestions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<i64>().ok())
        .unwrap_or(50)
        .min(200)
        .max(1);

    let pod_filter = params.get("pod_id");

    let rows = if let Some(pod_id) = pod_filter {
        sqlx::query_as::<_, (String, String, String, Option<String>, String, String, String, i32, String)>(
            "SELECT id, pod_id, sim_type, error_context, suggestion, model, source, dismissed, created_at \
             FROM ai_suggestions WHERE pod_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(pod_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, String, String, Option<String>, String, String, String, i32, String)>(
            "SELECT id, pod_id, sim_type, error_context, suggestion, model, source, dismissed, created_at \
             FROM ai_suggestions ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
    };

    match rows {
        Ok(suggestions) => {
            let list: Vec<Value> = suggestions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0,
                        "pod_id": s.1,
                        "sim_type": s.2,
                        "error_context": s.3,
                        "suggestion": s.4,
                        "model": s.5,
                        "source": s.6,
                        "dismissed": s.7 != 0,
                        "created_at": s.8,
                    })
                })
                .collect();
            Json(json!({ "suggestions": list }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn dismiss_ai_suggestion(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match sqlx::query("UPDATE ai_suggestions SET dismissed = 1 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "status": "dismissed" })),
        Ok(_) => Json(json!({ "error": "Suggestion not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── AI Training Management ─────────────────────────────────────────────────

/// GET /ai/training/stats — training pair counts, avg quality, top keywords.
async fn ai_training_stats(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let db = &state.db;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_training_pairs")
        .fetch_one(db).await.unwrap_or(0);

    let avg_quality: f64 = sqlx::query_scalar(
        "SELECT COALESCE(AVG(quality_score), 0.0) FROM ai_training_pairs"
    ).fetch_one(db).await.unwrap_or(0.0);

    let by_source = sqlx::query_as::<_, (String, i64)>(
        "SELECT source, COUNT(*) as cnt FROM ai_training_pairs GROUP BY source ORDER BY cnt DESC"
    ).fetch_all(db).await.unwrap_or_default();

    let top_used = sqlx::query_as::<_, (String, i64)>(
        "SELECT query_text, use_count FROM ai_training_pairs ORDER BY use_count DESC LIMIT 10"
    ).fetch_all(db).await.unwrap_or_default();

    Json(json!({
        "total": total,
        "avg_quality_score": (avg_quality * 100.0).round() / 100.0,
        "by_source": by_source.iter().map(|(s, c)| json!({"source": s, "count": c})).collect::<Vec<_>>(),
        "top_used": top_used.iter().map(|(q, u)| json!({"query": q, "use_count": u})).collect::<Vec<_>>(),
    }))
}

/// GET /ai/training/pairs — paginated list for review.
async fn ai_training_pairs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit: i64 = params.get("limit").and_then(|v| v.parse().ok()).unwrap_or(20);
    let offset: i64 = params.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0);
    let source_filter = params.get("source");

    let (pairs, total) = if let Some(src) = source_filter {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String)>(
            "SELECT id, query_text, response_text, source, model, quality_score, use_count, created_at \
             FROM ai_training_pairs WHERE source = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
        ).bind(src).bind(limit).bind(offset).fetch_all(&state.db).await.unwrap_or_default();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_training_pairs WHERE source = ?"
        ).bind(src).fetch_one(&state.db).await.unwrap_or(0);

        (rows, total)
    } else {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String)>(
            "SELECT id, query_text, response_text, source, model, quality_score, use_count, created_at \
             FROM ai_training_pairs ORDER BY created_at DESC LIMIT ? OFFSET ?",
        ).bind(limit).bind(offset).fetch_all(&state.db).await.unwrap_or_default();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_training_pairs"
        ).fetch_one(&state.db).await.unwrap_or(0);

        (rows, total)
    };

    Json(json!({
        "total": total,
        "limit": limit,
        "offset": offset,
        "pairs": pairs.iter().map(|(id, q, r, src, model, quality, use_count, created)| json!({
            "id": id,
            "query": q,
            "response": r,
            "source": src,
            "model": model,
            "quality_score": quality,
            "use_count": use_count,
            "created_at": created,
        })).collect::<Vec<_>>(),
    }))
}

#[derive(Debug, Deserialize)]
struct TrainingImportItem {
    query: String,
    response: String,
    #[serde(default = "default_source")]
    source: String,
    #[serde(default = "default_quality")]
    quality_score: i64,
}
fn default_source() -> String { "import".to_string() }
fn default_quality() -> i64 { 1 }

/// POST /ai/training/import — bulk import training pairs.
async fn ai_training_import(
    State(state): State<Arc<AppState>>,
    Json(pairs): Json<Vec<TrainingImportItem>>,
) -> Json<Value> {
    let mut inserted = 0u32;
    let mut skipped = 0u32;

    for item in &pairs {
        // Reuse the same log_training_pair logic but with quality_score support
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        item.query.hash(&mut hasher);
        let qhash = format!("{:x}", hasher.finish());

        let keywords = crate::ai::extract_keywords_pub(&item.query);
        let id = uuid::Uuid::new_v4().to_string();

        let result = sqlx::query(
            "INSERT INTO ai_training_pairs \
             (id, query_hash, query_text, query_keywords, response_text, source, model, quality_score) \
             SELECT ?, ?, ?, ?, ?, ?, 'import', ? \
             WHERE NOT EXISTS (SELECT 1 FROM ai_training_pairs WHERE query_hash = ?)",
        )
        .bind(&id)
        .bind(&qhash)
        .bind(&item.query)
        .bind(&keywords)
        .bind(&item.response)
        .bind(&item.source)
        .bind(item.quality_score)
        .bind(&qhash)
        .execute(&state.db)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => inserted += 1,
            _ => skipped += 1,
        }
    }

    Json(json!({
        "imported": inserted,
        "skipped": skipped,
        "total_submitted": pairs.len(),
    }))
}

// ─── Wallet (staff-facing) ───────────────────────────────────────────────────

async fn wallet_bonus_tiers() -> Json<Value> {
    Json(json!({
        "tiers": [
            { "min_paise": 200_000, "bonus_pct": 10 },
            { "min_paise": 400_000, "bonus_pct": 20 },
        ]
    }))
}

async fn get_wallet(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    match wallet::get_wallet_info(&state, &driver_id).await {
        Ok(Some(info)) => Json(json!({ "wallet": info })),
        Ok(None) => Json(json!({ "wallet": null })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Debug, Deserialize)]
struct TopupRequest {
    amount_paise: i64,
    method: String, // cash, card, upi
    notes: Option<String>,
    staff_id: Option<String>,
}

async fn topup_wallet(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Json(req): Json<TopupRequest>,
) -> Json<Value> {
    let txn_type = match req.method.as_str() {
        "cash" => "topup_cash",
        "card" => "topup_card",
        "upi" => "topup_upi",
        _ => "topup_cash",
    };

    let mut new_balance = match wallet::credit(
        &state,
        &driver_id,
        req.amount_paise,
        txn_type,
        None,
        req.notes.as_deref(),
        req.staff_id.as_deref(),
    )
    .await
    {
        Ok(b) => b,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Bonus credit tiers
    let bonus_pct = if req.amount_paise >= 400_000 { 20 }
        else if req.amount_paise >= 200_000 { 10 }
        else { 0 };

    let bonus_paise = if bonus_pct > 0 {
        let bp = req.amount_paise * bonus_pct / 100;
        let _ = wallet::credit(
            &state,
            &driver_id,
            bp,
            "bonus",
            None,
            Some(&format!("{}% topup bonus on {} credits", bonus_pct, req.amount_paise / 100)),
            req.staff_id.as_deref(),
        )
        .await;
        new_balance = wallet::get_balance(&state, &driver_id).await.unwrap_or(new_balance);
        bp
    } else {
        0
    };

    Json(json!({
        "status": "ok",
        "new_balance_paise": new_balance,
        "bonus_paise": bonus_paise,
    }))
}

#[derive(Debug, Deserialize)]
struct DebitRequest {
    amount_paise: i64,
    reason: String, // cafe, merchandise, penalty, etc.
    reference_id: Option<String>,
    notes: Option<String>,
}

async fn debit_wallet_manual(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Json(req): Json<DebitRequest>,
) -> Json<Value> {
    let txn_type = format!("debit_{}", req.reason);

    match wallet::debit(
        &state,
        &driver_id,
        req.amount_paise,
        &txn_type,
        req.reference_id.as_deref(),
        req.notes.as_deref(),
    )
    .await
    {
        Ok((new_balance, txn_id)) => Json(json!({
            "status": "ok",
            "new_balance_paise": new_balance,
            "txn_id": txn_id,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn wallet_transactions(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit = params.get("limit").and_then(|l| l.parse().ok()).unwrap_or(50i64);
    let txns = wallet::get_transactions(&state, &driver_id, limit).await;
    Json(json!({ "transactions": txns }))
}

#[derive(Debug, Deserialize)]
struct RefundRequest {
    amount_paise: i64,
    notes: Option<String>,
    reference_id: Option<String>,
}

async fn refund_wallet(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Json(req): Json<RefundRequest>,
) -> Json<Value> {
    match wallet::credit(
        &state,
        &driver_id,
        req.amount_paise,
        "refund_manual",
        req.reference_id.as_deref(),
        req.notes.as_deref(),
        None,
    )
    .await
    {
        Ok(new_balance) => Json(json!({
            "status": "ok",
            "new_balance_paise": new_balance,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Customer Wallet ────────────────────────────────────────────────────────

async fn customer_wallet(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match wallet::get_wallet_info(&state, &driver_id).await {
        Ok(Some(info)) => Json(json!({ "wallet": info })),
        Ok(None) => Json(json!({ "wallet": { "driver_id": driver_id, "balance_paise": 0, "total_credited_paise": 0, "total_debited_paise": 0 } })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_wallet_transactions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let limit = params.get("limit").and_then(|l| l.parse().ok()).unwrap_or(50i64);
    let txns = wallet::get_transactions(&state, &driver_id, limit).await;
    Json(json!({ "transactions": txns }))
}

// ─── Customer Experiences ───────────────────────────────────────────────────

async fn customer_experiences(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i64, String, i64)>(
        "SELECT e.id, e.name, e.game, e.track, e.car, e.car_class, e.duration_minutes, e.start_type, e.sort_order
         FROM kiosk_experiences e WHERE e.is_active = 1 ORDER BY e.sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    // Also fetch pricing tiers for the client
    let tiers = sqlx::query_as::<_, (String, String, i64, i64, bool, i64)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial, sort_order
         FROM pricing_tiers WHERE is_active = 1 ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match (rows, tiers) {
        (Ok(experiences), Ok(tiers)) => {
            let exp_list: Vec<Value> = experiences.iter().map(|e| json!({
                "id": e.0, "name": e.1, "game": e.2, "track": e.3,
                "car": e.4, "car_class": e.5, "duration_minutes": e.6,
                "start_type": e.7, "sort_order": e.8,
            })).collect();

            let tier_list: Vec<Value> = tiers.iter().map(|t| json!({
                "id": t.0, "name": t.1, "duration_minutes": t.2,
                "price_paise": t.3, "is_trial": t.4, "sort_order": t.5,
            })).collect();

            Json(json!({ "experiences": exp_list, "pricing_tiers": tier_list }))
        }
        _ => Json(json!({ "error": "Failed to load experiences" })),
    }
}

// ─── AC Catalog ─────────────────────────────────────────────────────────────

async fn customer_ac_catalog() -> Json<Value> {
    Json(catalog::get_catalog())
}

// ─── Customer Booking ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CustomBookingOptions {
    game: String,
    game_mode: Option<String>,
    track: String,
    car: String,
    difficulty: String,
    transmission: String,
}

#[derive(Debug, Deserialize)]
struct BookSessionRequest {
    experience_id: Option<String>,
    pricing_tier_id: String,
    custom: Option<CustomBookingOptions>,
}

async fn customer_book_session(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<BookSessionRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Validate pricing tier and get price
    let tier = match sqlx::query_as::<_, (String, String, i64, i64, bool)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&req.pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return Json(json!({ "error": "Invalid pricing tier" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    let is_trial = tier.4;
    let price_paise = tier.3;

    // Handle trial booking
    if is_trial {
        let has_used = sqlx::query_as::<_, (bool,)>(
            "SELECT COALESCE(has_used_trial, 0) FROM drivers WHERE id = ?",
        )
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await;

        match has_used {
            Ok(Some((true,))) => return Json(json!({ "error": "Free trial already used" })),
            Ok(None) => return Json(json!({ "error": "Driver not found" })),
            Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
            _ => {} // OK to proceed
        }
    } else {
        // Validate wallet balance for non-trial
        let balance = match wallet::get_balance(&state, &driver_id).await {
            Ok(b) => b,
            Err(e) => return Json(json!({ "error": e })),
        };

        if balance < price_paise {
            return Json(json!({
                "error": "Insufficient wallet balance",
                "balance_paise": balance,
                "required_paise": price_paise,
            }));
        }
    }

    // Check if driver already has an active reservation
    if let Some(existing) = pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await {
        return Json(json!({
            "error": "You already have an active reservation",
            "reservation_id": existing.id,
            "pod_id": existing.pod_id,
        }));
    }

    // Find idle pod
    let pod_id = match pod_reservation::find_idle_pod(&state).await {
        Some(id) => id,
        None => return Json(json!({ "error": "No pods available right now. Please try again shortly." })),
    };

    // Get pod number for display
    let pod_number = {
        let pods = state.pods.read().await;
        pods.get(&pod_id).map(|p| p.number).unwrap_or(0)
    };

    // Debit wallet (skip for trial)
    let (wallet_txn_id, wallet_debit) = if !is_trial && price_paise > 0 {
        match wallet::debit(
            &state,
            &driver_id,
            price_paise,
            "debit_session",
            None, // reference_id set after billing session created
            Some(&format!("{} on Pod {}", tier.1, pod_number)),
        )
        .await
        {
            Ok((_, txn_id)) => (Some(txn_id), Some(price_paise)),
            Err(e) => return Json(json!({ "error": e })),
        }
    } else {
        (None, None)
    };

    // Create pod reservation
    let reservation_id = match pod_reservation::create_reservation(&state, &driver_id, &pod_id).await {
        Ok(id) => id,
        Err(e) => {
            // Refund if we already debited
            if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
                let _ = wallet::refund(&state, &driver_id, amount, None, Some("Booking failed — auto-refund")).await;
            }
            return Json(json!({ "error": e }));
        }
    };

    // Validate: must have either experience_id or custom, not both, not neither
    if req.experience_id.is_none() && req.custom.is_none() {
        // Refund if we already debited
        if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
            let _ = wallet::refund(&state, &driver_id, amount, None, Some("Booking failed — auto-refund")).await;
        }
        let _ = pod_reservation::end_reservation(&state, &reservation_id).await;
        return Json(json!({ "error": "Either experience_id or custom must be provided" }));
    }

    // Build custom launch args if custom booking
    let custom_launch_args = req.custom.as_ref().map(|c| {
        // Get driver name for launch args
        let driver_name_for_args = "Driver"; // Will be set properly by launch_or_assist
        catalog::build_custom_launch_args(
            &c.car, &c.track, driver_name_for_args, &c.difficulty, &c.transmission,
        ).to_string()
    });

    // For custom bookings, also embed game info in the launch args
    let custom_launch_args = if let Some(ref args) = custom_launch_args {
        if let Some(ref c) = req.custom {
            let mut parsed: serde_json::Value = serde_json::from_str(args).unwrap_or_default();
            parsed["game"] = serde_json::json!(c.game);
            parsed["game_mode"] = serde_json::json!(c.game_mode.as_deref().unwrap_or("single"));
            Some(parsed.to_string())
        } else {
            custom_launch_args
        }
    } else {
        None
    };

    // Create auth token (PIN type) for this pod
    let experience_id = req.experience_id.clone();
    let auth_token = match auth::create_auth_token(
        &state,
        pod_id.clone(),
        driver_id.clone(),
        req.pricing_tier_id.clone(),
        "pin".to_string(),
        None, // custom_price_paise
        None, // custom_duration_minutes
        experience_id,
        custom_launch_args,
    )
    .await
    {
        Ok(token_info) => token_info,
        Err(e) => {
            // Cleanup: end reservation + refund
            let _ = pod_reservation::end_reservation(&state, &reservation_id).await;
            if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
                let _ = wallet::refund(&state, &driver_id, amount, None, Some("Booking failed — auto-refund")).await;
            }
            return Json(json!({ "error": format!("Failed to create auth: {}", e) }));
        }
    };

    Json(json!({
        "status": "booked",
        "reservation_id": reservation_id,
        "pod_id": pod_id,
        "pod_number": pod_number,
        "pin": auth_token.token,
        "allocated_seconds": auth_token.allocated_seconds,
        "wallet_debit_paise": wallet_debit,
        "wallet_txn_id": wallet_txn_id,
    }))
}

async fn customer_active_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let reservation = pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await;

    match reservation {
        Some(res) => {
            // Get pod number
            let pod_number = {
                let pods = state.pods.read().await;
                pods.get(&res.pod_id).map(|p| p.number).unwrap_or(0)
            };

            // Check if there's an active billing session on this pod
            let active_billing = {
                let timers = state.billing.active_timers.read().await;
                timers.get(&res.pod_id).map(|t| t.to_info())
            };

            Json(json!({
                "reservation": res,
                "pod_number": pod_number,
                "active_billing": active_billing,
            }))
        }
        None => Json(json!({ "reservation": null })),
    }
}

async fn customer_end_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let reservation = match pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await {
        Some(r) => r,
        None => return Json(json!({ "error": "No active reservation" })),
    };

    // End any active billing on this pod
    {
        let timers = state.billing.active_timers.read().await;
        if let Some(timer) = timers.get(&reservation.pod_id) {
            let session_id = timer.session_id.clone();
            drop(timers);

            // Proportional refund
            let billing = sqlx::query_as::<_, (i64, i64, Option<i64>)>(
                "SELECT allocated_seconds, driving_seconds, wallet_debit_paise FROM billing_sessions WHERE id = ?",
            )
            .bind(&session_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some((allocated, driving, Some(debit))) = billing {
                if debit > 0 && driving < allocated {
                    let remaining = allocated - driving;
                    let refund_amount = (remaining * debit) / allocated;
                    if refund_amount > 0 {
                        let _ = wallet::refund(
                            &state,
                            &driver_id,
                            refund_amount,
                            Some(&session_id),
                            Some("Early end — proportional refund"),
                        )
                        .await;
                    }
                }
            }

            billing::end_billing_session_public(&state, &session_id, rc_common::types::BillingSessionStatus::EndedEarly).await;
        }
    }

    // End the reservation
    let _ = pod_reservation::end_reservation(&state, &reservation.id).await;

    Json(json!({ "status": "ok" }))
}

// ─── Continue Session (Multi-Sub-Session) ───────────────────────────────────

#[derive(Debug, Deserialize)]
struct ContinueSessionRequest {
    experience_id: String,
    pricing_tier_id: String,
}

async fn customer_continue_session(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<ContinueSessionRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Must have an active reservation
    let reservation = match pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await {
        Some(r) => r,
        None => return Json(json!({ "error": "No active reservation. Book a new session instead." })),
    };

    // Must not have active billing on this pod
    {
        let timers = state.billing.active_timers.read().await;
        if timers.contains_key(&reservation.pod_id) {
            return Json(json!({ "error": "A session is still active on this pod" }));
        }
    }

    // Get pricing tier
    let tier = match sqlx::query_as::<_, (String, String, i64, i64, bool)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&req.pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return Json(json!({ "error": "Invalid pricing tier" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    let price_paise = tier.3;

    // Debit wallet
    if price_paise > 0 {
        let balance = match wallet::get_balance(&state, &driver_id).await {
            Ok(b) => b,
            Err(e) => return Json(json!({ "error": e })),
        };

        if balance < price_paise {
            return Json(json!({
                "error": "Insufficient wallet balance",
                "balance_paise": balance,
                "required_paise": price_paise,
            }));
        }

        match wallet::debit(
            &state,
            &driver_id,
            price_paise,
            "debit_session",
            None,
            Some(&format!("Continue: {}", tier.1)),
        )
        .await
        {
            Ok(_) => {}
            Err(e) => return Json(json!({ "error": e })),
        }
    }

    // Touch reservation
    pod_reservation::touch_reservation(&state, &reservation.id).await;

    // Start billing session directly (skip auth token — customer is already at pod)
    let billing_session_id = match billing::start_billing_session(
        &state,
        reservation.pod_id.clone(),
        driver_id.clone(),
        req.pricing_tier_id.clone(),
        None,
        None,
        None, // customer-initiated continue
    )
    .await
    {
        Ok(id) => id,
        Err(reason) => {
            // Refund on failure
            if price_paise > 0 {
                let _ = wallet::refund(&state, &driver_id, price_paise, None, Some("Continue failed — auto-refund")).await;
            }
            return Json(json!({ "error": reason }));
        }
    };

    // Link billing session to reservation and record wallet debit
    let _ = sqlx::query(
        "UPDATE billing_sessions SET reservation_id = ?, wallet_debit_paise = ? WHERE id = ?",
    )
    .bind(&reservation.id)
    .bind(price_paise)
    .bind(&billing_session_id)
    .execute(&state.db)
    .await;

    // Auto-launch game
    let exp = sqlx::query_as::<_, (String, String, String)>(
        "SELECT game, track, car FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&req.experience_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((game, track, car)) = exp {
        let sim_type = match game.as_str() {
            "assetto_corsa" | "ac" => SimType::AssettoCorsa,
            "iracing" => SimType::IRacing,
            "f1_25" | "f1" => SimType::F125,
            "le_mans_ultimate" | "lmu" => SimType::LeMansUltimate,
            "forza" => SimType::Forza,
            _ => SimType::AssettoCorsa,
        };

        // Check if this game supports auto-spawn
        let needs_assistance = matches!(sim_type, SimType::F125);

        let agent_senders = state.agent_senders.read().await;
        if needs_assistance {
            // Send assistance screen instead of launching
            if let Some(sender) = agent_senders.get(&reservation.pod_id) {
                let _ = sender.send(rc_common::protocol::CoreToAgentMessage::ShowAssistanceScreen {
                    driver_name: driver_id.clone(),
                    message: "A team member is on the way to help launch your game.".to_string(),
                }).await;
            }
            let _ = state.dashboard_tx.send(DashboardEvent::AssistanceNeeded {
                pod_id: reservation.pod_id.clone(),
                driver_name: driver_id.clone(),
                game: game.clone(),
                reason: "Game requires manual launch".to_string(),
            });
        } else {
            let launch_args = serde_json::json!({ "car": car, "track": track, "driver": "Driver" }).to_string();
            if let Some(sender) = agent_senders.get(&reservation.pod_id) {
                let _ = sender.send(rc_common::protocol::CoreToAgentMessage::LaunchGame {
                    sim_type,
                    launch_args: Some(launch_args),
                }).await;
            }
        }

        // Update billing session with experience info
        let _ = sqlx::query(
            "UPDATE billing_sessions SET experience_id = ?, car = ?, track = ?, sim_type = ? WHERE id = ?",
        )
        .bind(&req.experience_id)
        .bind(&car)
        .bind(&track)
        .bind(&game)
        .bind(&billing_session_id)
        .execute(&state.db)
        .await;
    }

    Json(json!({
        "status": "ok",
        "billing_session_id": billing_session_id,
        "reservation_id": reservation.id,
        "pod_id": reservation.pod_id,
    }))
}

// ─── Kiosk ──────────────────────────────────────────────────────────────────

async fn list_kiosk_experiences(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i64, String, Option<String>, i64, i64)>(
        "SELECT id, name, game, track, car, car_class, duration_minutes, start_type, ac_preset_id, sort_order, is_active
         FROM kiosk_experiences WHERE is_active = 1 ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(experiences) => {
            let list: Vec<Value> = experiences
                .iter()
                .map(|e| {
                    json!({
                        "id": e.0, "name": e.1, "game": e.2,
                        "track": e.3, "car": e.4, "car_class": e.5,
                        "duration_minutes": e.6, "start_type": e.7,
                        "ac_preset_id": e.8, "sort_order": e.9,
                        "is_active": e.10 != 0,
                    })
                })
                .collect();
            Json(json!({ "experiences": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_kiosk_experience(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("New Experience");
    let game = body.get("game").and_then(|v| v.as_str()).unwrap_or("assetto_corsa");
    let track = body.get("track").and_then(|v| v.as_str()).unwrap_or("spa");
    let car = body.get("car").and_then(|v| v.as_str()).unwrap_or("ks_ferrari_sf15t");
    let car_class = body.get("car_class").and_then(|v| v.as_str());
    let duration_minutes = body.get("duration_minutes").and_then(|v| v.as_i64()).unwrap_or(30);
    let start_type = body.get("start_type").and_then(|v| v.as_str()).unwrap_or("pitlane");
    let ac_preset_id = body.get("ac_preset_id").and_then(|v| v.as_str());
    let sort_order = body.get("sort_order").and_then(|v| v.as_i64()).unwrap_or(10);

    let result = sqlx::query(
        "INSERT INTO kiosk_experiences (id, name, game, track, car, car_class, duration_minutes, start_type, ac_preset_id, sort_order)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(game)
    .bind(track)
    .bind(car)
    .bind(car_class)
    .bind(duration_minutes)
    .bind(start_type)
    .bind(ac_preset_id)
    .bind(sort_order)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "name": name })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_kiosk_experience(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i64, String, Option<String>, i64, i64)>(
        "SELECT id, name, game, track, car, car_class, duration_minutes, start_type, ac_preset_id, sort_order, is_active
         FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(e)) => Json(json!({
            "id": e.0, "name": e.1, "game": e.2,
            "track": e.3, "car": e.4, "car_class": e.5,
            "duration_minutes": e.6, "start_type": e.7,
            "ac_preset_id": e.8, "sort_order": e.9,
            "is_active": e.10 != 0,
        })),
        Ok(None) => Json(json!({ "error": "Experience not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_kiosk_experience(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(v) = body.get("name").and_then(|v| v.as_str()) {
        updates.push("name = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("game").and_then(|v| v.as_str()) {
        updates.push("game = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("track").and_then(|v| v.as_str()) {
        updates.push("track = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("car").and_then(|v| v.as_str()) {
        updates.push("car = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("car_class").and_then(|v| v.as_str()) {
        updates.push("car_class = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("duration_minutes").and_then(|v| v.as_i64()) {
        updates.push("duration_minutes = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("start_type").and_then(|v| v.as_str()) {
        updates.push("start_type = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("ac_preset_id").and_then(|v| v.as_str()) {
        updates.push("ac_preset_id = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("sort_order").and_then(|v| v.as_i64()) {
        updates.push("sort_order = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("is_active").and_then(|v| v.as_bool()) {
        updates.push("is_active = ?");
        binds.push(if v { "1".to_string() } else { "0".to_string() });
    }

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" }));
    }

    let query = format!("UPDATE kiosk_experiences SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(r) if r.rows_affected() == 0 => Json(json!({ "error": "Experience not found" })),
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_kiosk_experience(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match sqlx::query("UPDATE kiosk_experiences SET is_active = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "ok": true })),
        Ok(_) => Json(json!({ "error": "Experience not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_kiosk_settings(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT key, value FROM kiosk_settings",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(settings) => {
            let mut map = serde_json::Map::new();
            for (key, value) in &settings {
                map.insert(key.clone(), json!(value));
            }
            Json(json!({ "settings": map }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_kiosk_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let obj = match body.as_object() {
        Some(o) => o,
        None => return Json(json!({ "error": "Expected a JSON object of key-value pairs" })),
    };

    let mut updated = 0;
    for (key, value) in obj {
        let val_str = match value.as_str() {
            Some(s) => s.to_string(),
            None => value.to_string(),
        };

        let result = sqlx::query(
            "INSERT INTO kiosk_settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(&val_str)
        .execute(&state.db)
        .await;

        if result.is_ok() {
            updated += 1;
        }
    }

    // Broadcast updated settings to all connected agents (with per-pod blanking override)
    if updated > 0 {
        let settings_map: std::collections::HashMap<String, String> = obj
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or(&v.to_string()).to_string()))
            .collect();
        state.broadcast_settings(&settings_map).await;
    }

    Json(json!({ "ok": true, "updated": updated }))
}

// ─── Cloud Action Queue Endpoints ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CreateActionRequest {
    action_type: String,
    payload: Value,
}

/// POST /actions — create a new action for the venue to pick up.
/// Auth: x-terminal-secret header (same as sync endpoints).
async fn create_action(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateActionRequest>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let payload_str = serde_json::to_string(&body.payload).unwrap_or_else(|_| "{}".to_string());

    let result = sqlx::query(
        "INSERT INTO action_queue (id, action_type, payload, status, created_at)
         VALUES (?, ?, ?, 'pending', datetime('now'))",
    )
    .bind(&id)
    .bind(&body.action_type)
    .bind(&payload_str)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Action queue: created {} ({})", id, body.action_type);
            Json(json!({ "ok": true, "id": id, "action_type": body.action_type }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to create action: {}", e) })),
    }
}

/// GET /actions/pending — returns all pending actions for the venue to process.
/// Auth: x-terminal-secret header.
async fn pending_actions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, action_type, payload, created_at
         FROM action_queue
         WHERE status = 'pending'
         ORDER BY created_at ASC
         LIMIT 50",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let actions: Vec<Value> = rows
                .iter()
                .map(|(id, action_type, payload, created_at)| {
                    let payload_val: Value =
                        serde_json::from_str(payload).unwrap_or(json!({}));
                    // Build the PendingCloudAction format expected by venue action_queue.rs
                    json!({
                        "id": id,
                        "action": {
                            "action_type": action_type,
                            "payload": payload_val,
                        },
                        "created_at": created_at,
                    })
                })
                .collect();

            // Mark returned actions as processing to avoid re-delivery
            for (id, _, _, _) in &rows {
                let _ = sqlx::query(
                    "UPDATE action_queue SET status = 'processing', processed_at = datetime('now') WHERE id = ?",
                )
                .bind(id)
                .execute(&state.db)
                .await;
            }

            Json(json!({ "actions": actions }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to fetch actions: {}", e) })),
    }
}

/// POST /actions/{id}/ack — venue acknowledges a processed action.
/// Auth: x-terminal-secret header.
async fn ack_action(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let status = body
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("completed");
    let error = body.get("error").and_then(|v| v.as_str());

    let result = sqlx::query(
        "UPDATE action_queue SET status = ?, error = ?, acked_at = datetime('now') WHERE id = ?",
    )
    .bind(status)
    .bind(error)
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::info!("Action queue: acked {} → {}", id, status);
            Json(json!({ "ok": true, "id": id, "status": status }))
        }
        Ok(_) => Json(json!({ "error": "Action not found" })),
        Err(e) => Json(json!({ "error": format!("Failed to ack: {}", e) })),
    }
}

/// GET /actions/history — recent action history for debugging.
/// Auth: x-terminal-secret header.
async fn action_history(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let limit: i64 = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);

    let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>, String, Option<String>, Option<String>)>(
        "SELECT id, action_type, payload, status, error, created_at, processed_at, acked_at
         FROM action_queue
         ORDER BY created_at DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let actions: Vec<Value> = rows
                .iter()
                .map(|(id, action_type, payload, status, error, created_at, processed_at, acked_at)| {
                    json!({
                        "id": id,
                        "action_type": action_type,
                        "payload": serde_json::from_str::<Value>(payload).unwrap_or(json!({})),
                        "status": status,
                        "error": error,
                        "created_at": created_at,
                        "processed_at": processed_at,
                        "acked_at": acked_at,
                    })
                })
                .collect();
            Json(json!({ "actions": actions, "total": actions.len() }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to fetch history: {}", e) })),
    }
}

// ─── Cloud Sync Endpoints ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SyncChangesQuery {
    since: Option<String>,
    tables: Option<String>,
    limit: Option<i64>,
}

async fn sync_changes(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<SyncChangesQuery>,
) -> Json<Value> {
    // Require terminal secret for sync endpoint (exposes customer PII)
    if let Some(secret) = state.config.cloud.terminal_secret.as_deref() {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided != Some(secret) {
            return Json(serde_json::json!({ "error": "Unauthorized" }));
        }
    }
    // Normalize ISO timestamps (2026-03-07T23:48:38Z) to SQLite format (2026-03-07 23:48:38)
    // SQLite's datetime('now') uses space, but sync_state stores ISO with 'T'.
    // String comparison: space (0x20) < 'T' (0x54), so "2026-03-07 23:59" < "2026-03-07T00:00"
    // Without normalization, updated records are never returned after first sync cycle.
    let since = params
        .since
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
        .replace('T', " ")
        .trim_end_matches('Z')
        .trim_end_matches('+')
        .split('+')
        .next()
        .unwrap_or("1970-01-01 00:00:00")
        .to_string();
    let tables: Vec<&str> = params
        .tables
        .as_deref()
        .unwrap_or("drivers,wallets,pricing_tiers,kiosk_experiences")
        .split(',')
        .map(|s| s.trim())
        .collect();
    let limit = params.limit.unwrap_or(500);

    let mut result = json!({});

    for table in &tables {
        match *table {
            "drivers" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'customer_id', customer_id,
                        'name', name, 'email', email, 'phone', phone,
                        'steam_guid', steam_guid, 'iracing_id', iracing_id,
                        'avatar_url', avatar_url, 'total_laps', total_laps,
                        'total_time_ms', total_time_ms,
                        'has_used_trial', COALESCE(has_used_trial, 0),
                        'pin_hash', pin_hash, 'phone_verified', COALESCE(phone_verified, 0),
                        'dob', dob, 'waiver_signed', COALESCE(waiver_signed, 0),
                        'waiver_signed_at', waiver_signed_at, 'waiver_version', waiver_version,
                        'guardian_name', guardian_name, 'guardian_phone', guardian_phone,
                        'registration_completed', COALESCE(registration_completed, 0),
                        'signature_data', signature_data,
                        'created_at', created_at, 'updated_at', updated_at
                    ) FROM drivers
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["drivers"] = json!(items);
                }
            }
            "wallets" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'driver_id', w.driver_id, 'balance_paise', w.balance_paise,
                        'total_credited_paise', w.total_credited_paise,
                        'total_debited_paise', w.total_debited_paise,
                        'updated_at', w.updated_at,
                        'phone', d.phone, 'email', d.email
                    ) FROM wallets w
                    LEFT JOIN drivers d ON d.id = w.driver_id
                    WHERE w.updated_at > ?
                    ORDER BY w.updated_at ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["wallets"] = json!(items);
                }
            }
            "pricing_tiers" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'name', name, 'duration_minutes', duration_minutes,
                        'price_paise', price_paise, 'is_trial', is_trial,
                        'is_active', is_active, 'sort_order', sort_order,
                        'created_at', created_at, 'updated_at', updated_at
                    ) FROM pricing_tiers
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["pricing_tiers"] = json!(items);
                }
            }
            "kiosk_experiences" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'name', name, 'game', game, 'track', track,
                        'car', car, 'car_class', car_class,
                        'duration_minutes', duration_minutes, 'start_type', start_type,
                        'ac_preset_id', ac_preset_id, 'sort_order', sort_order,
                        'is_active', is_active,
                        'created_at', created_at, 'updated_at', updated_at
                    ) FROM kiosk_experiences
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["kiosk_experiences"] = json!(items);
                }
            }
            "pricing_rules" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'rule_name', rule_name, 'rule_type', rule_type,
                        'day_of_week', day_of_week, 'hour_start', hour_start,
                        'hour_end', hour_end, 'multiplier', multiplier,
                        'flat_adjustment_paise', flat_adjustment_paise,
                        'is_active', is_active
                    ) FROM pricing_rules",
                )
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["pricing_rules"] = json!(items);
                }
            }
            "kiosk_settings" => {
                // kiosk_settings is a key-value table, return as a flat object
                let rows = sqlx::query_as::<_, (String, String)>(
                    "SELECT key, value FROM kiosk_settings",
                )
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let mut settings = json!({});
                    for (key, value) in &rows {
                        settings[key] = json!(value);
                    }
                    result["kiosk_settings"] = settings;
                }
            }
            _ => {}
        }
    }

    result["synced_at"] = json!(chrono::Utc::now().to_rfc3339());
    Json(result)
}

/// POST /sync/push — venue pushes data to cloud
async fn sync_push(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    // Auth check
    if let Some(secret) = state.config.cloud.terminal_secret.as_deref() {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided != Some(secret) {
            return Json(json!({ "error": "Unauthorized" }));
        }
    }

    let mut total = 0u64;

    // Upsert laps
    if let Some(laps) = body.get("laps").and_then(|v| v.as_array()) {
        for lap in laps {
            let id = lap.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO laps (id, session_id, driver_id, pod_id, sim_type, track, car,
                    lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
                 ON CONFLICT(id) DO NOTHING",
            )
            .bind(id)
            .bind(lap.get("session_id").and_then(|v| v.as_str()))
            .bind(lap.get("driver_id").and_then(|v| v.as_str()))
            .bind(lap.get("pod_id").and_then(|v| v.as_str()))
            .bind(lap.get("sim_type").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(lap.get("track").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(lap.get("car").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(lap.get("lap_number").and_then(|v| v.as_i64()))
            .bind(lap.get("lap_time_ms").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(lap.get("sector1_ms").and_then(|v| v.as_i64()))
            .bind(lap.get("sector2_ms").and_then(|v| v.as_i64()))
            .bind(lap.get("sector3_ms").and_then(|v| v.as_i64()))
            .bind(lap.get("valid").and_then(|v| v.as_i64()).unwrap_or(1))
            .bind(lap.get("created_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
    }

    // Upsert track_records (best lap per track+car)
    if let Some(records) = body.get("track_records").and_then(|v| v.as_array()) {
        for rec in records {
            let track = rec.get("track").and_then(|v| v.as_str()).unwrap_or_default();
            let car = rec.get("car").and_then(|v| v.as_str()).unwrap_or_default();
            if track.is_empty() || car.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO track_records (track, car, driver_id, best_lap_ms, lap_id, achieved_at)
                 VALUES (?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(track, car) DO UPDATE SET
                    driver_id = CASE WHEN excluded.best_lap_ms < track_records.best_lap_ms
                        THEN excluded.driver_id ELSE track_records.driver_id END,
                    best_lap_ms = MIN(excluded.best_lap_ms, track_records.best_lap_ms),
                    lap_id = CASE WHEN excluded.best_lap_ms < track_records.best_lap_ms
                        THEN excluded.lap_id ELSE track_records.lap_id END,
                    achieved_at = CASE WHEN excluded.best_lap_ms < track_records.best_lap_ms
                        THEN excluded.achieved_at ELSE track_records.achieved_at END",
            )
            .bind(track)
            .bind(car)
            .bind(rec.get("driver_id").and_then(|v| v.as_str()))
            .bind(rec.get("best_lap_ms").and_then(|v| v.as_i64()).unwrap_or(i64::MAX))
            .bind(rec.get("lap_id").and_then(|v| v.as_str()))
            .bind(rec.get("achieved_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
    }

    // Upsert personal_bests
    if let Some(pbs) = body.get("personal_bests").and_then(|v| v.as_array()) {
        for pb in pbs {
            let driver_id = pb.get("driver_id").and_then(|v| v.as_str()).unwrap_or_default();
            let track = pb.get("track").and_then(|v| v.as_str()).unwrap_or_default();
            let car = pb.get("car").and_then(|v| v.as_str()).unwrap_or_default();
            if driver_id.is_empty() || track.is_empty() || car.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO personal_bests (driver_id, track, car, best_lap_ms, lap_id, achieved_at)
                 VALUES (?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(driver_id, track, car) DO UPDATE SET
                    best_lap_ms = MIN(excluded.best_lap_ms, personal_bests.best_lap_ms),
                    lap_id = CASE WHEN excluded.best_lap_ms < personal_bests.best_lap_ms
                        THEN excluded.lap_id ELSE personal_bests.lap_id END,
                    achieved_at = CASE WHEN excluded.best_lap_ms < personal_bests.best_lap_ms
                        THEN excluded.achieved_at ELSE personal_bests.achieved_at END",
            )
            .bind(driver_id)
            .bind(track)
            .bind(car)
            .bind(pb.get("best_lap_ms").and_then(|v| v.as_i64()).unwrap_or(i64::MAX))
            .bind(pb.get("lap_id").and_then(|v| v.as_str()))
            .bind(pb.get("achieved_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
    }

    // Upsert billing_sessions
    if let Some(sessions) = body.get("billing_sessions").and_then(|v| v.as_array()) {
        for s in sessions {
            let id = s.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id,
                    allocated_seconds, driving_seconds, status, custom_price_paise, notes,
                    started_at, ended_at, created_at, experience_id, car, track, sim_type)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)
                 ON CONFLICT(id) DO UPDATE SET
                    driving_seconds = excluded.driving_seconds,
                    status = excluded.status,
                    ended_at = excluded.ended_at",
            )
            .bind(id)
            .bind(s.get("driver_id").and_then(|v| v.as_str()))
            .bind(s.get("pod_id").and_then(|v| v.as_str()))
            .bind(s.get("pricing_tier_id").and_then(|v| v.as_str()))
            .bind(s.get("allocated_seconds").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(s.get("driving_seconds").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(s.get("status").and_then(|v| v.as_str()).unwrap_or("pending"))
            .bind(s.get("custom_price_paise").and_then(|v| v.as_i64()))
            .bind(s.get("notes").and_then(|v| v.as_str()))
            .bind(s.get("started_at").and_then(|v| v.as_str()))
            .bind(s.get("ended_at").and_then(|v| v.as_str()))
            .bind(s.get("created_at").and_then(|v| v.as_str()))
            .bind(s.get("experience_id").and_then(|v| v.as_str()))
            .bind(s.get("car").and_then(|v| v.as_str()))
            .bind(s.get("track").and_then(|v| v.as_str()))
            .bind(s.get("sim_type").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
    }

    // Merge driver updates from venue (venue-owned fields only)
    if let Some(drivers) = body.get("drivers").and_then(|v| v.as_array()) {
        for d in drivers {
            let id = d.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }

            // Only update venue-owned fields, never overwrite cloud-owned fields (name, email, phone)
            let venue_updated = d.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");

            // Check if cloud has a newer update for this driver
            let cloud_ts: Option<(Option<String>,)> = sqlx::query_as(
                "SELECT updated_at FROM drivers WHERE id = ?",
            )
            .bind(id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            // Only apply venue fields if venue's updated_at is newer
            let should_apply = match &cloud_ts {
                Some((Some(ts),)) => venue_updated > ts.as_str(),
                Some((None,)) => true,
                None => false, // Driver doesn't exist on cloud — skip partial update
            };

            if should_apply {
                let r = sqlx::query(
                    "UPDATE drivers SET
                        has_used_trial = MAX(COALESCE(has_used_trial, 0), ?),
                        total_laps = MAX(COALESCE(total_laps, 0), ?),
                        total_time_ms = MAX(COALESCE(total_time_ms, 0), ?),
                        registration_completed = MAX(COALESCE(registration_completed, 0), ?),
                        waiver_signed = MAX(COALESCE(waiver_signed, 0), ?),
                        waiver_signed_at = COALESCE(?, waiver_signed_at),
                        waiver_version = COALESCE(?, waiver_version),
                        updated_at = ?
                     WHERE id = ?",
                )
                .bind(d.get("has_used_trial").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("total_laps").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("total_time_ms").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("registration_completed").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("waiver_signed").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("waiver_signed_at").and_then(|v| v.as_str()))
                .bind(d.get("waiver_version").and_then(|v| v.as_str()))
                .bind(venue_updated)
                .bind(id)
                .execute(&state.db)
                .await;
                if r.is_ok() { total += 1; }
            }
        }
    }

    // Upsert pods (static config + live status)
    if let Some(pods) = body.get("pods").and_then(|v| v.as_array()) {
        for pod in pods {
            let id = pod.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let number = pod.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
            let name = pod.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
            let status = pod.get("status").and_then(|v| v.as_str()).unwrap_or("offline");

            // Update DB
            let _ = sqlx::query(
                "INSERT INTO pods (id, number, name, ip_address, sim_type, status, last_seen)
                 VALUES (?1,?2,?3,?4,?5,?6,datetime('now'))
                 ON CONFLICT(id) DO UPDATE SET
                    status = excluded.status,
                    ip_address = excluded.ip_address,
                    last_seen = datetime('now')",
            )
            .bind(id)
            .bind(number)
            .bind(name)
            .bind(pod.get("ip_address").and_then(|v| v.as_str()))
            .bind(pod.get("sim_type").and_then(|v| v.as_str()).unwrap_or("assetto_corsa"))
            .bind(status)
            .execute(&state.db)
            .await;

            // Update in-memory pod map so PWA/dashboard sees live status
            let pod_info = rc_common::types::PodInfo {
                id: id.to_string(),
                number: number as u32,
                name: name.to_string(),
                ip_address: pod.get("ip_address").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                mac_address: pod.get("mac_address").and_then(|v| v.as_str()).map(|s| s.to_string()),
                sim_type: pod.get("sim_type").and_then(|v| v.as_str())
                    .and_then(|s| serde_json::from_value(json!(s)).ok())
                    .unwrap_or(rc_common::types::SimType::AssettoCorsa),
                status: serde_json::from_value(json!(status))
                    .unwrap_or(rc_common::types::PodStatus::Offline),
                current_driver: pod.get("current_driver").and_then(|v| v.as_str()).map(|s| s.to_string()),
                current_session_id: pod.get("current_session_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                last_seen: Some(chrono::Utc::now()),
                driving_state: None,
                billing_session_id: pod.get("billing_session_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                game_state: None,
                current_game: None,
            };
            state.pods.write().await.insert(id.to_string(), pod_info.clone());
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod_info));
            total += 1;
        }
    }

    // Upsert wallets (venue pushes balances after billing debits)
    // Handles ID mismatch: if direct driver_id doesn't match, resolve by phone/email
    if let Some(wallets) = body.get("wallets").and_then(|v| v.as_array()) {
        for w in wallets {
            let driver_id = w.get("driver_id").and_then(|v| v.as_str()).unwrap_or_default();
            if driver_id.is_empty() { continue; }

            let balance = w.get("balance_paise").and_then(|v| v.as_i64()).unwrap_or(0);
            let credited = w.get("total_credited_paise").and_then(|v| v.as_i64()).unwrap_or(0);
            let debited = w.get("total_debited_paise").and_then(|v| v.as_i64()).unwrap_or(0);
            let updated = w.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");

            // Try direct driver_id match first
            let r = sqlx::query(
                "UPDATE wallets SET
                    balance_paise = ?, total_credited_paise = ?,
                    total_debited_paise = ?, updated_at = ?
                 WHERE driver_id = ?",
            )
            .bind(balance).bind(credited).bind(debited).bind(updated)
            .bind(driver_id)
            .execute(&state.db)
            .await;

            let rows = r.as_ref().map(|r| r.rows_affected()).unwrap_or(0);
            if rows > 0 {
                total += 1;
                continue;
            }

            // ID didn't match — try to find local driver by phone or email
            let phone = w.get("phone").and_then(|v| v.as_str()).unwrap_or("");
            let email = w.get("email").and_then(|v| v.as_str()).unwrap_or("");

            let resolved: Option<(String,)> = if !phone.is_empty() {
                sqlx::query_as("SELECT id FROM drivers WHERE phone = ?")
                    .bind(phone)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten()
            } else if !email.is_empty() {
                sqlx::query_as("SELECT id FROM drivers WHERE email = ?")
                    .bind(email)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            };

            if let Some((local_id,)) = resolved {
                let r2 = sqlx::query(
                    "UPDATE wallets SET
                        balance_paise = ?, total_credited_paise = ?,
                        total_debited_paise = ?, updated_at = ?
                     WHERE driver_id = ?",
                )
                .bind(balance).bind(credited).bind(debited).bind(updated)
                .bind(&local_id)
                .execute(&state.db)
                .await;
                if r2.is_ok() {
                    tracing::info!("Wallet sync: resolved {} -> {} by phone/email", driver_id, local_id);
                    total += 1;
                }
            }
        }
    }

    tracing::info!("Sync push: upserted {} records", total);
    Json(json!({ "ok": true, "upserted": total }))
}

async fn sync_health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let driver_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM drivers")
        .fetch_one(&state.db)
        .await
        .map(|r| r.0)
        .unwrap_or(0);

    let sync_states = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT table_name, last_synced_at, last_sync_count FROM sync_state ORDER BY table_name",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let sync_info: Vec<Value> = sync_states
        .iter()
        .map(|(table, last, count)| {
            json!({ "table": table, "last_synced_at": last, "last_sync_count": count })
        })
        .collect();

    Json(json!({
        "status": "ok",
        "drivers": driver_count,
        "cloud_sync_enabled": state.config.cloud.enabled,
        "cloud_api_url": state.config.cloud.api_url,
        "sync_state": sync_info,
    }))
}

// ─── Terminal (remote command execution) ─────────────────────────────────────

async fn check_terminal_auth(state: &AppState, headers: &axum::http::HeaderMap) -> Result<(), String> {
    // 1. Check PIN session token (x-terminal-session header)
    if let Some(token) = headers.get("x-terminal-session").and_then(|v| v.to_str().ok()) {
        let sessions = state.terminal_sessions.read().await;
        if let Some(expiry) = sessions.get(token) {
            if *expiry > chrono::Utc::now() {
                return Ok(());
            }
        }
    }

    // 2. Check legacy shared secret (x-terminal-secret header — for cloud polling)
    let secret = state.config.cloud.terminal_secret.as_deref();
    if let Some(secret) = secret {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided == Some(secret) {
            return Ok(());
        }
    }

    // 3. If no secret AND no pin configured, allow (local dev)
    if state.config.cloud.terminal_secret.is_none() && state.config.cloud.terminal_pin.is_none() {
        return Ok(());
    }

    Err("Unauthorized. Use POST /terminal/auth with your PIN.".to_string())
}

#[derive(Deserialize)]
struct TerminalAuthRequest {
    pin: String,
}

/// POST /terminal/auth — authenticate with PIN, returns a 24h session token
async fn terminal_auth(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TerminalAuthRequest>,
) -> Json<Value> {
    let expected = state.config.cloud.terminal_pin.as_deref();
    match expected {
        None => {
            return Json(json!({ "error": "Terminal PIN not configured on server." }));
        }
        Some(pin) => {
            if req.pin != pin {
                tracing::warn!("Terminal auth failed — wrong PIN");
                return Json(json!({ "error": "Invalid PIN." }));
            }
        }
    }

    // Generate session token valid for 24 hours
    let token = uuid::Uuid::new_v4().to_string();
    let expiry = chrono::Utc::now() + chrono::Duration::hours(24);

    // Clean up expired sessions while we're here
    let mut sessions = state.terminal_sessions.write().await;
    let now = chrono::Utc::now();
    sessions.retain(|_, exp| *exp > now);
    sessions.insert(token.clone(), expiry);
    drop(sessions);

    tracing::info!("Terminal session created (expires {})", expiry.format("%Y-%m-%d %H:%M UTC"));

    Json(json!({
        "session": token,
        "expires_at": expiry.to_rfc3339(),
    }))
}

#[derive(Deserialize)]
struct TerminalSubmitRequest {
    cmd: String,
    timeout_ms: Option<i64>,
}

#[derive(Deserialize)]
struct TerminalResultRequest {
    exit_code: Option<i64>,
    stdout: Option<String>,
    stderr: Option<String>,
}

#[derive(Deserialize)]
struct TerminalListQuery {
    limit: Option<i64>,
}

async fn terminal_submit(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<TerminalSubmitRequest>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let timeout_ms = req.timeout_ms.unwrap_or(30000).min(120000);

    let result = sqlx::query(
        "INSERT INTO terminal_commands (id, cmd, status, timeout_ms) VALUES (?, ?, 'pending', ?)",
    )
    .bind(&id)
    .bind(&req.cmd)
    .bind(timeout_ms)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Terminal command queued: {} ({})", id, req.cmd);
            Json(json!({ "status": "queued", "id": id }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn terminal_list(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<TerminalListQuery>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let limit = params.limit.unwrap_or(50).min(200);

    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'cmd', cmd, 'status', status,
            'exit_code', exit_code, 'stdout', stdout, 'stderr', stderr,
            'timeout_ms', timeout_ms,
            'created_at', created_at, 'started_at', started_at, 'completed_at', completed_at
        ) FROM terminal_commands
        ORDER BY created_at DESC
        LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let commands: Vec<Value> = rows
                .iter()
                .filter_map(|r| serde_json::from_str(&r.0).ok())
                .collect();
            Json(json!({ "commands": commands }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn terminal_pending(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'cmd', cmd, 'timeout_ms', timeout_ms, 'created_at', created_at
        ) FROM terminal_commands
        WHERE status = 'pending'
        ORDER BY created_at ASC
        LIMIT 10",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let commands: Vec<Value> = rows
                .iter()
                .filter_map(|r| serde_json::from_str(&r.0).ok())
                .collect();
            Json(json!({ "commands": commands }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn terminal_result(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<TerminalResultRequest>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let status = if req.exit_code == Some(124) { "timeout" }
        else if req.exit_code.is_some() && req.exit_code != Some(0) { "failed" }
        else { "completed" };

    let result = sqlx::query(
        "UPDATE terminal_commands SET
            status = ?, exit_code = ?, stdout = ?, stderr = ?, completed_at = datetime('now')
         WHERE id = ?",
    )
    .bind(status)
    .bind(req.exit_code)
    .bind(&req.stdout)
    .bind(&req.stderr)
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::info!("Terminal command {} completed ({})", id, status);
            Json(json!({ "status": "ok" }))
        }
        Ok(_) => Json(json!({ "error": "Command not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── Employee Endpoints ──────────────────────────────────────────────────

/// GET /employee/daily-pin — returns today's 4-digit debug PIN (employee-only, JWT auth)
async fn employee_daily_pin(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Verify employee flag
    if !auth::is_employee(&state, &driver_id).await {
        return Json(json!({ "error": "Access denied. Employee account required." }));
    }

    let pin = auth::todays_debug_pin(&state.config.auth.jwt_secret);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    Json(json!({
        "pin": pin,
        "valid_date": today,
        "note": "4-digit PIN valid until midnight UTC. Enter on any pod lock screen to unlock debug mode."
    }))
}

#[derive(Debug, Deserialize)]
struct EmployeeDebugUnlockRequest {
    pin: String,
    pod_id: String,
}

/// POST /employee/debug-unlock — unlock a specific pod in debug mode
async fn employee_debug_unlock(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EmployeeDebugUnlockRequest>,
) -> Json<Value> {
    match auth::validate_employee_pin_kiosk(&state, req.pin, Some(req.pod_id.clone())).await {
        Ok(_) => Json(json!({
            "status": "ok",
            "pod_id": req.pod_id,
            "mode": "debug",
            "message": "Pod unlocked in debug mode. Content Manager access enabled."
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Staff ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct StaffValidatePinRequest {
    pin: String,
}

async fn staff_validate_pin(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StaffValidatePinRequest>,
) -> Json<Value> {
    let result = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM staff_members WHERE pin = ? AND is_active = 1",
    )
    .bind(&req.pin)
    .fetch_optional(&state.db)
    .await;

    match result {
        Ok(Some((id, name))) => {
            let _ = sqlx::query(
                "UPDATE staff_members SET last_login_at = datetime('now') WHERE id = ?",
            )
            .bind(&id)
            .execute(&state.db)
            .await;

            Json(json!({
                "status": "ok",
                "staff_id": id,
                "staff_name": name,
            }))
        }
        Ok(None) => Json(json!({ "error": "Invalid staff PIN" })),
        Err(e) => Json(json!({ "error": format!("Database error: {}", e) })),
    }
}

#[derive(Debug, Deserialize)]
struct CreateStaffRequest {
    name: String,
    phone: String,
    pin: String,
}

async fn create_staff(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateStaffRequest>,
) -> Json<Value> {
    let id = format!("staff_{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("x"));

    match sqlx::query(
        "INSERT INTO staff_members (id, name, phone, pin) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.phone)
    .bind(&req.pin)
    .execute(&state.db)
    .await
    {
        Ok(_) => Json(json!({ "status": "ok", "id": id, "name": req.name })),
        Err(e) => Json(json!({ "error": format!("{}", e) })),
    }
}

async fn list_staff(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, bool, Option<String>)>(
        "SELECT id, name, phone, pin, is_active, last_login_at FROM staff_members ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let staff: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, phone, pin, active, last_login)| {
            json!({
                "id": id,
                "name": name,
                "phone": phone,
                "pin": pin,
                "is_active": active,
                "last_login_at": last_login,
            })
        })
        .collect();

    Json(json!({ "staff": staff }))
}

// ─── Friends ──────────────────────────────────────────────────────────────

async fn customer_friends(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::list_friends(&state, &driver_id).await {
        Ok(list) => Json(json!({ "friends": list })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_friend_requests(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::list_friend_requests(&state, &driver_id).await {
        Ok((incoming, outgoing)) => Json(json!({
            "incoming": incoming,
            "outgoing": outgoing,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_send_friend_request(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let identifier = match req.get("identifier").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'identifier' (phone or customer_id)" })),
    };

    match friends::send_friend_request(&state, &driver_id, &identifier).await {
        Ok(request_id) => Json(json!({ "status": "ok", "request_id": request_id })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_accept_friend_request(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(request_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::accept_friend_request(&state, &request_id, &driver_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_reject_friend_request(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(request_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::reject_friend_request(&state, &request_id, &driver_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_remove_friend(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(friend_driver_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::remove_friend(&state, &driver_id, &friend_driver_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_set_presence(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let presence = match req.get("presence").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => return Json(json!({ "error": "Missing 'presence' (online/hidden)" })),
    };

    match friends::set_presence(&state, &driver_id, &presence).await {
        Ok(()) => Json(json!({ "status": "ok", "presence": presence })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Multiplayer ──────────────────────────────────────────────────────────

async fn customer_book_multiplayer(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let experience_id = match req.get("experience_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'experience_id'" })),
    };

    let pricing_tier_id = match req.get("pricing_tier_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'pricing_tier_id'" })),
    };

    let friend_ids: Vec<String> = match req.get("friend_ids").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        None => return Json(json!({ "error": "Missing 'friend_ids' array" })),
    };

    if friend_ids.is_empty() {
        return Json(json!({ "error": "Need at least one friend for multiplayer" }));
    }

    match multiplayer::book_multiplayer(&state, &driver_id, &experience_id, &pricing_tier_id, friend_ids).await {
        Ok(info) => Json(json!({ "status": "ok", "group_session": info })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_group_session(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match multiplayer::get_active_group_session(&state, &driver_id).await {
        Ok(Some(info)) => Json(json!({ "group_session": info })),
        Ok(None) => Json(json!({ "group_session": null })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_accept_group_invite(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(group_session_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match multiplayer::accept_group_invite(&state, &group_session_id, &driver_id).await {
        Ok(member) => Json(json!({ "status": "ok", "member": member })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_decline_group_invite(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(group_session_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match multiplayer::decline_group_invite(&state, &group_session_id, &driver_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Telemetry ────────────────────────────────────────────────────────────

async fn customer_telemetry(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Find the pod this driver is actively billing on
    let active_pod: Option<(String,)> = sqlx::query_as(
        "SELECT pod_id FROM billing_sessions WHERE driver_id = ? AND status = 'active' ORDER BY started_at DESC LIMIT 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let pod_id = match active_pod {
        Some((pid,)) => pid,
        None => return Json(json!({ "error": "No active session" })),
    };

    // Get latest telemetry sample for this pod
    let sample: Option<(String, String)> = sqlx::query_as(
        "SELECT data, sampled_at FROM telemetry_samples WHERE pod_id = ? ORDER BY sampled_at DESC LIMIT 1",
    )
    .bind(&pod_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match sample {
        Some((data, sampled_at)) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(&data) {
                Json(json!({
                    "pod_id": pod_id,
                    "telemetry": parsed,
                    "sampled_at": sampled_at,
                }))
            } else {
                Json(json!({ "pod_id": pod_id, "telemetry": data, "sampled_at": sampled_at }))
            }
        }
        None => Json(json!({ "pod_id": pod_id, "telemetry": null })),
    }
}

// ─── Shareable Session Report ────────────────────────────────────────────────

async fn customer_session_share(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Fetch billing session
    let session = sqlx::query_as::<_, (
        String, String, String, i64, i64, String, i64,
        Option<String>, Option<String>, Option<String>, Option<String>,
    )>(
        "SELECT bs.id, bs.pod_id, pt.name, bs.allocated_seconds, bs.driving_seconds,
                bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at, bs.car, bs.track
         FROM billing_sessions bs
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.id = ? AND bs.driver_id = ?",
    )
    .bind(&id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Get driver name
    let driver_name: String = sqlx::query_as::<_, (String,)>(
        "SELECT name FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|r| r.0)
    .unwrap_or_else(|| "Driver".to_string());

    // Get laps
    let laps = sqlx::query_as::<_, (i64, i64, Option<i64>, Option<i64>, Option<i64>, bool, String, String)>(
        "SELECT lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, track, car
         FROM laps WHERE session_id = ? AND driver_id = ?
         ORDER BY lap_number ASC",
    )
    .bind(&id)
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let total_laps = laps.len();
    let valid_laps: Vec<_> = laps.iter().filter(|l| l.5).collect();
    let best_lap_ms = valid_laps.iter().map(|l| l.1).min();
    let avg_lap_ms = if !valid_laps.is_empty() {
        Some(valid_laps.iter().map(|l| l.1).sum::<i64>() / valid_laps.len() as i64)
    } else {
        None
    };
    let consistency = if valid_laps.len() >= 3 {
        let mean = valid_laps.iter().map(|l| l.1 as f64).sum::<f64>() / valid_laps.len() as f64;
        let variance = valid_laps.iter().map(|l| {
            let diff = l.1 as f64 - mean;
            diff * diff
        }).sum::<f64>() / valid_laps.len() as f64;
        let std_dev = variance.sqrt();
        let cv = std_dev / mean * 100.0;
        // Lower CV = more consistent. <2% = excellent, <5% = good, <10% = average
        Some(json!({
            "std_dev_ms": std_dev.round() as i64,
            "coefficient_of_variation": (cv * 100.0).round() / 100.0,
            "rating": if cv < 2.0 { "Excellent" } else if cv < 5.0 { "Good" } else if cv < 10.0 { "Average" } else { "Inconsistent" },
        }))
    } else {
        None
    };

    // Determine track/car from laps or session
    let track = laps.first().map(|l| l.6.clone()).or(session.10.clone()).unwrap_or_default();
    let car = laps.first().map(|l| l.7.clone()).or(session.9.clone()).unwrap_or_default();

    // Percentile ranking: how does this best lap compare to all laps on this track+car?
    let percentile = if let Some(best) = best_lap_ms {
        if !track.is_empty() && !car.is_empty() {
            let total_count: Option<(i64,)> = sqlx::query_as(
                "SELECT COUNT(DISTINCT driver_id) FROM laps WHERE track = ? AND car = ? AND valid = 1",
            )
            .bind(&track)
            .bind(&car)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            let faster_count: Option<(i64,)> = sqlx::query_as(
                "SELECT COUNT(DISTINCT driver_id) FROM (
                    SELECT driver_id, MIN(lap_time_ms) as best
                    FROM laps WHERE track = ? AND car = ? AND valid = 1
                    GROUP BY driver_id
                ) WHERE best < ?",
            )
            .bind(&track)
            .bind(&car)
            .bind(best)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            match (total_count, faster_count) {
                (Some((total,)), Some((faster,))) if total > 1 => {
                    Some(((total - faster) as f64 / total as f64 * 100.0).round() as u32)
                }
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    };

    // Track record for comparison
    let track_record: Option<(i64, String)> = if !track.is_empty() && !car.is_empty() {
        sqlx::query_as(
            "SELECT tr.best_lap_ms, d.name FROM track_records tr
             JOIN drivers d ON tr.driver_id = d.id
             WHERE tr.track = ? AND tr.car = ?",
        )
        .bind(&track)
        .bind(&car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    // Personal best for this track+car
    let personal_best: Option<(i64,)> = if !track.is_empty() && !car.is_empty() {
        sqlx::query_as(
            "SELECT best_lap_ms FROM personal_bests WHERE driver_id = ? AND track = ? AND car = ?",
        )
        .bind(&driver_id)
        .bind(&track)
        .bind(&car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    // Improvement: compare first valid lap to best valid lap
    let improvement_ms = if valid_laps.len() >= 2 {
        let first = valid_laps.first().unwrap().1;
        let best = best_lap_ms.unwrap();
        Some(first - best)
    } else {
        None
    };

    // Build share card data
    let driving_minutes = session.4 / 60;

    Json(json!({
        "share_report": {
            "driver_name": driver_name,
            "track": track,
            "car": car,
            "date": session.7,
            "driving_time_seconds": session.4,
            "driving_time_display": format!("{}m {}s", driving_minutes, session.4 % 60),
            "total_laps": total_laps,
            "valid_laps": valid_laps.len(),
            "best_lap_ms": best_lap_ms,
            "best_lap_display": best_lap_ms.map(|ms| format!("{}:{:02}.{:03}", ms / 60000, (ms % 60000) / 1000, ms % 1000)),
            "average_lap_ms": avg_lap_ms,
            "improvement_ms": improvement_ms,
            "consistency": consistency,
            "percentile_rank": percentile,
            "percentile_text": percentile.map(|p| format!("Top {}% of drivers", 100 - p.min(99))),
            "track_record": track_record.as_ref().map(|(ms, name)| json!({
                "time_ms": ms,
                "holder": name,
                "gap_ms": best_lap_ms.map(|b| b - ms),
            })),
            "personal_best_ms": personal_best.map(|pb| pb.0),
            "is_new_pb": personal_best.map(|pb| best_lap_ms == Some(pb.0)).unwrap_or(false),
            "laps": laps.iter().map(|l| json!({
                "lap": l.0, "time_ms": l.1,
                "s1": l.2, "s2": l.3, "s3": l.4,
                "valid": l.5,
            })).collect::<Vec<_>>(),
            "venue": "RacingPoint",
            "tagline": "May the Fastest Win.",
        }
    }))
}

// ─── Referral System ─────────────────────────────────────────────────────────

async fn customer_referral_code(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let code: Option<(String,)> = sqlx::query_as(
        "SELECT referral_code FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let referral_count: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM referrals WHERE referrer_id = ? AND reward_credited = 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Json(json!({
        "referral_code": code.and_then(|c| if c.0.is_empty() { None } else { Some(c.0) }),
        "successful_referrals": referral_count.map(|c| c.0).unwrap_or(0),
    }))
}

async fn customer_generate_referral_code(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Check if already has a code
    let existing: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT referral_code FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((Some(code),)) = &existing {
        if !code.is_empty() {
            return Json(json!({ "referral_code": code }));
        }
    }

    // Generate 6-char alphanumeric code
    use std::fmt::Write;
    let mut code = String::with_capacity(6);
    let chars = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    for _ in 0..6 {
        let idx = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos() as usize) % chars.len();
        let _ = write!(code, "{}", chars[idx] as char);
        // tiny spin to get different nanos
        std::hint::spin_loop();
    }

    let code = format!("RP{}", code);

    let _ = sqlx::query("UPDATE drivers SET referral_code = ? WHERE id = ?")
        .bind(&code)
        .bind(&driver_id)
        .execute(&state.db)
        .await;

    Json(json!({ "referral_code": code }))
}

async fn customer_redeem_referral(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let code = match body.get("code").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return Json(json!({ "error": "code required" })),
    };

    // Find referrer
    let referrer: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM drivers WHERE referral_code = ?",
    )
    .bind(code)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let referrer_id = match referrer {
        Some((id,)) => {
            if id == driver_id {
                return Json(json!({ "error": "Cannot redeem your own code" }));
            }
            id
        }
        None => return Json(json!({ "error": "Invalid referral code" })),
    };

    // Check not already referred
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM referrals WHERE referee_id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if existing.map(|e| e.0 > 0).unwrap_or(false) {
        return Json(json!({ "error": "Already used a referral code" }));
    }

    let referral_id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO referrals (id, referrer_id, referee_id, code, reward_credited)
         VALUES (?, ?, ?, ?, 0)",
    )
    .bind(&referral_id)
    .bind(&referrer_id)
    .bind(&driver_id)
    .bind(code)
    .execute(&state.db)
    .await;

    Json(json!({ "ok": true, "message": "Referral code applied! Rewards will be credited after your first session." }))
}

// ─── Coupons ─────────────────────────────────────────────────────────────────

async fn customer_apply_coupon(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let code = match body.get("code").and_then(|v| v.as_str()) {
        Some(c) => c.to_uppercase(),
        None => return Json(json!({ "error": "code required" })),
    };

    // Find coupon
    let coupon: Option<(String, String, f64, i64, Option<String>, Option<String>, Option<i64>, bool)> = sqlx::query_as(
        "SELECT id, coupon_type, value, max_uses, valid_from, valid_until, min_spend_paise, first_session_only
         FROM coupons WHERE code = ? AND active = 1",
    )
    .bind(&code)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let coupon = match coupon {
        Some(c) => c,
        None => return Json(json!({ "error": "Invalid or expired coupon code" })),
    };

    // Check usage count
    let used: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM coupon_redemptions WHERE coupon_id = ?",
    )
    .bind(&coupon.0)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if used.map(|u| u.0 >= coupon.3).unwrap_or(false) {
        return Json(json!({ "error": "Coupon has reached maximum uses" }));
    }

    // Check if already used by this driver
    let driver_used: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM coupon_redemptions WHERE coupon_id = ? AND driver_id = ?",
    )
    .bind(&coupon.0)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if driver_used.map(|u| u.0 > 0).unwrap_or(false) {
        return Json(json!({ "error": "You have already used this coupon" }));
    }

    // Return coupon details for the client to apply at checkout
    let discount_description = match coupon.1.as_str() {
        "percent" => format!("{}% off", coupon.2),
        "flat" => format!("₹{} off", coupon.2 as i64 / 100),
        "free_minutes" => format!("{} free minutes", coupon.2 as i64),
        _ => "Discount".to_string(),
    };

    Json(json!({
        "valid": true,
        "coupon_id": coupon.0,
        "coupon_type": coupon.1,
        "value": coupon.2,
        "description": discount_description,
        "min_spend_paise": coupon.6,
        "first_session_only": coupon.7,
    }))
}

// ─── Packages ────────────────────────────────────────────────────────────────

async fn customer_list_packages(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, i64, i64, i64, bool, Option<String>, Option<String>)>(
        "SELECT id, name, description, num_rigs, duration_minutes, price_paise,
                includes_cafe, day_restriction, hour_restriction
         FROM packages WHERE active = 1
         ORDER BY price_paise ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(packages) => {
            let list: Vec<Value> = packages.iter().map(|p| json!({
                "id": p.0,
                "name": p.1,
                "description": p.2,
                "num_rigs": p.3,
                "duration_minutes": p.4,
                "price_paise": p.5,
                "price_display": format!("₹{}", p.5 / 100),
                "includes_cafe": p.6,
                "day_restriction": p.7,
                "hour_restriction": p.8,
            })).collect();
            Json(json!({ "packages": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Memberships ─────────────────────────────────────────────────────────────

async fn customer_membership(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Get active membership
    let membership: Option<(String, String, String, f64, f64, String, bool, String)> = sqlx::query_as(
        "SELECT m.id, mt.name, mt.perks, m.hours_used, mt.hours_included,
                m.expires_at, m.auto_renew, m.status
         FROM memberships m
         JOIN membership_tiers mt ON m.tier_id = mt.id
         WHERE m.driver_id = ? AND m.status = 'active'
         ORDER BY m.created_at DESC LIMIT 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Get available tiers
    let tiers = sqlx::query_as::<_, (String, String, f64, i64, String)>(
        "SELECT id, name, hours_included, price_paise, perks
         FROM membership_tiers WHERE active = 1
         ORDER BY price_paise ASC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let tiers_json: Vec<Value> = tiers.iter().map(|t| {
        let perks: Value = serde_json::from_str(&t.4).unwrap_or(json!([]));
        json!({
            "id": t.0,
            "name": t.1,
            "hours_included": t.2,
            "price_paise": t.3,
            "price_display": format!("₹{}/month", t.3 / 100),
            "perks": perks,
        })
    }).collect();

    Json(json!({
        "membership": membership.map(|m| {
            let perks: Value = serde_json::from_str(&m.2).unwrap_or(json!([]));
            json!({
                "id": m.0,
                "tier_name": m.1,
                "perks": perks,
                "hours_used": m.3,
                "hours_included": m.4,
                "hours_remaining": (m.4 - m.3).max(0.0),
                "expires_at": m.5,
                "auto_renew": m.6,
                "status": m.7,
            })
        }),
        "available_tiers": tiers_json,
    }))
}

async fn customer_subscribe_membership(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let tier_id = match body.get("tier_id").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return Json(json!({ "error": "tier_id required" })),
    };

    // Check tier exists
    let tier: Option<(String, i64)> = sqlx::query_as(
        "SELECT name, price_paise FROM membership_tiers WHERE id = ? AND active = 1",
    )
    .bind(tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let tier = match tier {
        Some(t) => t,
        None => return Json(json!({ "error": "Invalid membership tier" })),
    };

    // Check no active membership
    let active: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM memberships WHERE driver_id = ? AND status = 'active'",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if active.map(|a| a.0 > 0).unwrap_or(false) {
        return Json(json!({ "error": "You already have an active membership" }));
    }

    let membership_id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO memberships (id, driver_id, tier_id, hours_used, expires_at, auto_renew, status)
         VALUES (?, ?, ?, 0, datetime('now', '+30 days'), 0, 'active')",
    )
    .bind(&membership_id)
    .bind(&driver_id)
    .bind(tier_id)
    .execute(&state.db)
    .await;

    Json(json!({
        "ok": true,
        "membership_id": membership_id,
        "tier_name": tier.0,
        "message": format!("Welcome to {} membership!", tier.0),
    }))
}

// ─── Public Leaderboard (No Auth Required) ───────────────────────────────────

async fn public_leaderboard(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // All-time track records across all tracks
    let records = sqlx::query_as::<_, (String, String, String, i64, String)>(
        "SELECT tr.track, tr.car, CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END, tr.best_lap_ms, tr.achieved_at
         FROM track_records tr
         JOIN drivers d ON tr.driver_id = d.id
         ORDER BY tr.achieved_at DESC",
    )
    .fetch_all(&state.db)
    .await;

    // Available tracks
    let tracks = sqlx::query_as::<_, (String, i64)>(
        "SELECT DISTINCT track, COUNT(*) as laps
         FROM laps WHERE valid = 1
         GROUP BY track
         ORDER BY laps DESC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Top drivers by total valid laps
    let top_drivers = sqlx::query_as::<_, (String, i64, Option<i64>)>(
        "SELECT CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END, COUNT(l.id) as lap_count, MIN(l.lap_time_ms) as fastest
         FROM laps l
         JOIN drivers d ON l.driver_id = d.id
         WHERE l.valid = 1
         GROUP BY l.driver_id
         ORDER BY lap_count DESC
         LIMIT 20",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Active time trial
    let time_trial = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, track, car, week_start, week_end
         FROM time_trials
         WHERE date('now') BETWEEN week_start AND week_end
         LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Json(json!({
        "records": records.unwrap_or_default().iter().map(|r| json!({
            "track": r.0, "car": r.1, "driver": r.2,
            "best_lap_ms": r.3,
            "best_lap_display": format!("{}:{:02}.{:03}", r.3 / 60000, (r.3 % 60000) / 1000, r.3 % 1000),
            "achieved_at": r.4,
        })).collect::<Vec<_>>(),
        "tracks": tracks.iter().map(|t| json!({
            "name": t.0, "total_laps": t.1,
        })).collect::<Vec<_>>(),
        "top_drivers": top_drivers.iter().enumerate().map(|(i, d)| json!({
            "position": i + 1,
            "name": d.0,
            "total_laps": d.1,
            "fastest_lap_ms": d.2,
        })).collect::<Vec<_>>(),
        "time_trial": time_trial.map(|tt| json!({
            "id": tt.0, "track": tt.1, "car": tt.2,
            "week_start": tt.3, "week_end": tt.4,
        })),
        "venue": "RacingPoint",
        "tagline": "May the Fastest Win.",
    }))
}

async fn public_track_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(track): Path<String>,
) -> Json<Value> {
    // Top 50 fastest laps on this track (best per driver per car)
    let records = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END, l.car, MIN(l.lap_time_ms), MAX(l.created_at)
         FROM laps l
         JOIN drivers d ON l.driver_id = d.id
         WHERE l.track = ? AND l.valid = 1
         GROUP BY l.driver_id, l.car
         ORDER BY MIN(l.lap_time_ms) ASC
         LIMIT 50",
    )
    .bind(&track)
    .fetch_all(&state.db)
    .await;

    // Track stats
    let stats: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT COUNT(*) as total_laps, COUNT(DISTINCT driver_id) as drivers, COUNT(DISTINCT car) as cars
         FROM laps WHERE track = ? AND valid = 1",
    )
    .bind(&track)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Json(json!({
        "track": track,
        "stats": stats.map(|s| json!({
            "total_laps": s.0,
            "unique_drivers": s.1,
            "unique_cars": s.2,
        })),
        "leaderboard": records.unwrap_or_default().iter().enumerate().map(|(i, r)| json!({
            "position": i + 1,
            "driver": r.0,
            "car": r.1,
            "best_lap_ms": r.2,
            "best_lap_display": format!("{}:{:02}.{:03}", r.2 / 60000, (r.2 % 60000) / 1000, r.2 % 1000),
            "achieved_at": r.3,
        })).collect::<Vec<_>>(),
    }))
}

async fn public_time_trial(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // Current week's time trial
    let trial = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, track, car, week_start, week_end
         FROM time_trials
         WHERE date('now') BETWEEN week_start AND week_end
         LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let trial = match trial {
        Some(t) => t,
        None => return Json(json!({ "time_trial": null, "message": "No active time trial this week" })),
    };

    // Leaderboard for this time trial (laps on this track+car this week)
    let entries = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END, MIN(l.lap_time_ms), COUNT(l.id)
         FROM laps l
         JOIN drivers d ON l.driver_id = d.id
         WHERE l.track = ? AND l.car = ? AND l.valid = 1
           AND l.created_at >= ? AND l.created_at < datetime(?, '+1 day')
         GROUP BY l.driver_id
         ORDER BY MIN(l.lap_time_ms) ASC
         LIMIT 20",
    )
    .bind(&trial.1)
    .bind(&trial.2)
    .bind(&trial.3)
    .bind(&trial.4)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(json!({
        "time_trial": {
            "id": trial.0,
            "track": trial.1,
            "car": trial.2,
            "week_start": trial.3,
            "week_end": trial.4,
        },
        "leaderboard": entries.iter().enumerate().map(|(i, e)| json!({
            "position": i + 1,
            "driver": e.0,
            "best_lap_ms": e.1,
            "best_lap_display": format!("{}:{:02}.{:03}", e.1 / 60000, (e.1 % 60000) / 1000, e.1 % 1000),
            "attempts": e.2,
        })).collect::<Vec<_>>(),
    }))
}

// ─── Dynamic Pricing Admin ───────────────────────────────────────────────────

async fn list_pricing_rules(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, Option<i64>, Option<i64>, Option<i64>, f64, i64, bool)>(
        "SELECT id, rule_type, day_of_week, hour_start, hour_end, multiplier, flat_adjustment_paise, active
         FROM pricing_rules ORDER BY rule_type, day_of_week, hour_start",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rules) => {
            let list: Vec<Value> = rules.iter().map(|r| json!({
                "id": r.0, "rule_type": r.1,
                "day_of_week": r.2, "hour_start": r.3, "hour_end": r.4,
                "multiplier": r.5, "flat_adjustment_paise": r.6, "active": r.7,
            })).collect();
            Json(json!({ "rules": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_pricing_rule(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let rule_type = body.get("rule_type").and_then(|v| v.as_str()).unwrap_or("custom");
    let multiplier = body.get("multiplier").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let flat_adj = body.get("flat_adjustment_paise").and_then(|v| v.as_i64()).unwrap_or(0);

    let result = sqlx::query(
        "INSERT INTO pricing_rules (id, rule_type, day_of_week, hour_start, hour_end, multiplier, flat_adjustment_paise, active)
         VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
    )
    .bind(&id)
    .bind(rule_type)
    .bind(body.get("day_of_week").and_then(|v| v.as_i64()))
    .bind(body.get("hour_start").and_then(|v| v.as_i64()))
    .bind(body.get("hour_end").and_then(|v| v.as_i64()))
    .bind(multiplier)
    .bind(flat_adj)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_pricing_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let result = sqlx::query(
        "UPDATE pricing_rules SET
            rule_type = COALESCE(?, rule_type),
            day_of_week = ?,
            hour_start = ?,
            hour_end = ?,
            multiplier = COALESCE(?, multiplier),
            flat_adjustment_paise = COALESCE(?, flat_adjustment_paise),
            active = COALESCE(?, active)
         WHERE id = ?",
    )
    .bind(body.get("rule_type").and_then(|v| v.as_str()))
    .bind(body.get("day_of_week").and_then(|v| v.as_i64()))
    .bind(body.get("hour_start").and_then(|v| v.as_i64()))
    .bind(body.get("hour_end").and_then(|v| v.as_i64()))
    .bind(body.get("multiplier").and_then(|v| v.as_f64()))
    .bind(body.get("flat_adjustment_paise").and_then(|v| v.as_i64()))
    .bind(body.get("active").and_then(|v| v.as_bool()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_pricing_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let _ = sqlx::query("DELETE FROM pricing_rules WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;
    Json(json!({ "ok": true }))
}

// ─── Coupons Admin ───────────────────────────────────────────────────────────

async fn list_coupons(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, f64, i64, Option<String>, Option<String>, Option<i64>, bool, bool)>(
        "SELECT id, code, coupon_type, value, max_uses, valid_from, valid_until, min_spend_paise, first_session_only, active
         FROM coupons ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(coupons) => {
            let list: Vec<Value> = coupons.iter().map(|c| json!({
                "id": c.0, "code": c.1, "coupon_type": c.2, "value": c.3,
                "max_uses": c.4, "valid_from": c.5, "valid_until": c.6,
                "min_spend_paise": c.7, "first_session_only": c.8, "active": c.9,
            })).collect();
            Json(json!({ "coupons": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_coupon(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let code = body.get("code").and_then(|v| v.as_str()).unwrap_or("").to_uppercase();
    if code.is_empty() {
        return Json(json!({ "error": "code required" }));
    }

    let result = sqlx::query(
        "INSERT INTO coupons (id, code, coupon_type, value, max_uses, valid_from, valid_until, min_spend_paise, first_session_only, active)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
    )
    .bind(&id)
    .bind(&code)
    .bind(body.get("coupon_type").and_then(|v| v.as_str()).unwrap_or("percent"))
    .bind(body.get("value").and_then(|v| v.as_f64()).unwrap_or(10.0))
    .bind(body.get("max_uses").and_then(|v| v.as_i64()).unwrap_or(100))
    .bind(body.get("valid_from").and_then(|v| v.as_str()))
    .bind(body.get("valid_until").and_then(|v| v.as_str()))
    .bind(body.get("min_spend_paise").and_then(|v| v.as_i64()))
    .bind(body.get("first_session_only").and_then(|v| v.as_bool()).unwrap_or(false))
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "code": code })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_coupon(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let result = sqlx::query(
        "UPDATE coupons SET
            code = COALESCE(?, code),
            coupon_type = COALESCE(?, coupon_type),
            value = COALESCE(?, value),
            max_uses = COALESCE(?, max_uses),
            valid_from = ?,
            valid_until = ?,
            min_spend_paise = ?,
            first_session_only = COALESCE(?, first_session_only),
            active = COALESCE(?, active)
         WHERE id = ?",
    )
    .bind(body.get("code").and_then(|v| v.as_str()))
    .bind(body.get("coupon_type").and_then(|v| v.as_str()))
    .bind(body.get("value").and_then(|v| v.as_f64()))
    .bind(body.get("max_uses").and_then(|v| v.as_i64()))
    .bind(body.get("valid_from").and_then(|v| v.as_str()))
    .bind(body.get("valid_until").and_then(|v| v.as_str()))
    .bind(body.get("min_spend_paise").and_then(|v| v.as_i64()))
    .bind(body.get("first_session_only").and_then(|v| v.as_bool()))
    .bind(body.get("active").and_then(|v| v.as_bool()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_coupon(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let _ = sqlx::query("DELETE FROM coupons WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;
    Json(json!({ "ok": true }))
}

// ─── Review Nudges ───────────────────────────────────────────────────────────

async fn pending_review_nudges(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT rn.id, rn.driver_id, d.name, d.phone
         FROM review_nudges rn
         JOIN drivers d ON rn.driver_id = d.id
         WHERE rn.sent_at IS NULL
         ORDER BY rn.created_at ASC
         LIMIT 50",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(nudges) => {
            let list: Vec<Value> = nudges.iter().map(|n| json!({
                "id": n.0, "driver_id": n.1, "driver_name": n.2, "phone": n.3,
            })).collect();
            Json(json!({ "nudges": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn mark_nudge_sent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let review_credited = body.get("review_credited").and_then(|v| v.as_bool()).unwrap_or(false);

    let _ = sqlx::query(
        "UPDATE review_nudges SET sent_at = datetime('now'), review_credited = ? WHERE id = ?",
    )
    .bind(review_credited)
    .bind(&id)
    .execute(&state.db)
    .await;

    // If they left a review, credit 50 credits (₹50)
    if review_credited {
        let driver: Option<(String,)> = sqlx::query_as(
            "SELECT driver_id FROM review_nudges WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some((driver_id,)) = driver {
            let _ = crate::wallet::credit(
                &state,
                &driver_id,
                5000,
                "review_reward",
                Some(&id),
                Some("Thank you for your Google review!"),
                None,
            )
            .await;
        }
    }

    Json(json!({ "ok": true }))
}

// ─── Time Trial Admin ────────────────────────────────────────────────────────

async fn list_time_trials(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, bool)>(
        "SELECT id, track, car, week_start, week_end, is_active
         FROM time_trials ORDER BY week_start DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(trials) => {
            let list: Vec<Value> = trials.iter().map(|t| json!({
                "id": t.0, "track": t.1, "car": t.2,
                "week_start": t.3, "week_end": t.4, "is_active": t.5,
            })).collect();
            Json(json!({ "time_trials": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_time_trial(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let track = match body.get("track").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return Json(json!({ "error": "track required" })),
    };
    let car = match body.get("car").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return Json(json!({ "error": "car required" })),
    };
    let week_start = match body.get("week_start").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return Json(json!({ "error": "week_start required (YYYY-MM-DD)" })),
    };
    let week_end = match body.get("week_end").and_then(|v| v.as_str()) {
        Some(e) => e,
        None => return Json(json!({ "error": "week_end required (YYYY-MM-DD)" })),
    };

    let result = sqlx::query(
        "INSERT INTO time_trials (id, track, car, week_start, week_end) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(track)
    .bind(car)
    .bind(week_start)
    .bind(week_end)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_time_trial(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let result = sqlx::query(
        "UPDATE time_trials SET
            track = COALESCE(?, track), car = COALESCE(?, car),
            week_start = COALESCE(?, week_start), week_end = COALESCE(?, week_end),
            is_active = COALESCE(?, is_active)
         WHERE id = ?",
    )
    .bind(body.get("track").and_then(|v| v.as_str()))
    .bind(body.get("car").and_then(|v| v.as_str()))
    .bind(body.get("week_start").and_then(|v| v.as_str()))
    .bind(body.get("week_end").and_then(|v| v.as_str()))
    .bind(body.get("is_active").and_then(|v| v.as_bool()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_time_trial(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let _ = sqlx::query("DELETE FROM time_trials WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;
    Json(json!({ "ok": true }))
}

// ─── Tournaments ─────────────────────────────────────────────────────────────

async fn list_tournaments(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i64, i64, i64, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, name, description, track, car, format, max_participants, entry_fee_paise, prize_pool_paise,
                status, registration_start, registration_end, event_date
         FROM tournaments ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(tournaments) => {
            let list: Vec<Value> = tournaments.iter().map(|t| json!({
                "id": t.0, "name": t.1, "description": t.2,
                "track": t.3, "car": t.4, "format": t.5,
                "max_participants": t.6, "entry_fee_paise": t.7,
                "prize_pool_paise": t.8, "status": t.9,
                "registration_start": t.10, "registration_end": t.11,
                "event_date": t.12,
            })).collect();
            Json(json!({ "tournaments": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_tournament(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => return Json(json!({ "error": "name required" })),
    };

    let result = sqlx::query(
        "INSERT INTO tournaments (id, name, description, track, car, format, max_participants, entry_fee_paise, prize_pool_paise, status, registration_start, registration_end, event_date, rules)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'upcoming', ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(body.get("description").and_then(|v| v.as_str()))
    .bind(body.get("track").and_then(|v| v.as_str()).unwrap_or(""))
    .bind(body.get("car").and_then(|v| v.as_str()).unwrap_or(""))
    .bind(body.get("format").and_then(|v| v.as_str()).unwrap_or("time_attack"))
    .bind(body.get("max_participants").and_then(|v| v.as_i64()).unwrap_or(16))
    .bind(body.get("entry_fee_paise").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(body.get("prize_pool_paise").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(body.get("registration_start").and_then(|v| v.as_str()))
    .bind(body.get("registration_end").and_then(|v| v.as_str()))
    .bind(body.get("event_date").and_then(|v| v.as_str()))
    .bind(body.get("rules").and_then(|v| v.as_str()))
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_tournament(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let tournament = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i64, i64, i64, String, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, name, description, track, car, format, max_participants, entry_fee_paise, prize_pool_paise,
                status, registration_start, registration_end, event_date, rules
         FROM tournaments WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let t = match tournament {
        Ok(Some(t)) => t,
        Ok(None) => return Json(json!({ "error": "Tournament not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Count registrations
    let reg_count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM tournament_registrations WHERE tournament_id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    Json(json!({
        "tournament": {
            "id": t.0, "name": t.1, "description": t.2,
            "track": t.3, "car": t.4, "format": t.5,
            "max_participants": t.6, "entry_fee_paise": t.7,
            "prize_pool_paise": t.8, "status": t.9,
            "registration_start": t.10, "registration_end": t.11,
            "event_date": t.12, "rules": t.13,
            "registered_count": reg_count,
        }
    }))
}

async fn update_tournament(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let result = sqlx::query(
        "UPDATE tournaments SET
            name = COALESCE(?, name), description = ?,
            track = COALESCE(?, track), car = COALESCE(?, car),
            format = COALESCE(?, format), max_participants = COALESCE(?, max_participants),
            entry_fee_paise = COALESCE(?, entry_fee_paise),
            prize_pool_paise = COALESCE(?, prize_pool_paise),
            status = COALESCE(?, status),
            registration_start = ?, registration_end = ?, event_date = ?,
            rules = ?
         WHERE id = ?",
    )
    .bind(body.get("name").and_then(|v| v.as_str()))
    .bind(body.get("description").and_then(|v| v.as_str()))
    .bind(body.get("track").and_then(|v| v.as_str()))
    .bind(body.get("car").and_then(|v| v.as_str()))
    .bind(body.get("format").and_then(|v| v.as_str()))
    .bind(body.get("max_participants").and_then(|v| v.as_i64()))
    .bind(body.get("entry_fee_paise").and_then(|v| v.as_i64()))
    .bind(body.get("prize_pool_paise").and_then(|v| v.as_i64()))
    .bind(body.get("status").and_then(|v| v.as_str()))
    .bind(body.get("registration_start").and_then(|v| v.as_str()))
    .bind(body.get("registration_end").and_then(|v| v.as_str()))
    .bind(body.get("event_date").and_then(|v| v.as_str()))
    .bind(body.get("rules").and_then(|v| v.as_str()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn tournament_registrations(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<i64>, String, Option<i64>)>(
        "SELECT tr.id, tr.driver_id, d.name, tr.seed, tr.status, tr.best_time_ms
         FROM tournament_registrations tr
         JOIN drivers d ON tr.driver_id = d.id
         WHERE tr.tournament_id = ?
         ORDER BY COALESCE(tr.seed, 9999), tr.created_at",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(regs) => {
            let list: Vec<Value> = regs.iter().map(|r| json!({
                "id": r.0, "driver_id": r.1, "driver_name": r.2,
                "seed": r.3, "status": r.4, "best_time_ms": r.5,
            })).collect();
            Json(json!({ "registrations": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn tournament_matches(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, i64, i64, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<String>, String)>(
        "SELECT tm.id, tm.round, tm.match_number,
                da.name, db.name,
                tm.time_a_ms, tm.time_b_ms, dw.name, tm.status
         FROM tournament_matches tm
         LEFT JOIN drivers da ON tm.driver_a = da.id
         LEFT JOIN drivers db ON tm.driver_b = db.id
         LEFT JOIN drivers dw ON tm.winner_id = dw.id
         WHERE tm.tournament_id = ?
         ORDER BY tm.round, tm.match_number",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(matches) => {
            let list: Vec<Value> = matches.iter().map(|m| json!({
                "id": m.0, "round": m.1, "match_number": m.2,
                "driver_a": m.3, "driver_b": m.4,
                "time_a_ms": m.5, "time_b_ms": m.6,
                "winner": m.7, "status": m.8,
            })).collect();
            Json(json!({ "matches": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn generate_bracket(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Get all registered drivers
    let regs = sqlx::query_as::<_, (String, Option<i64>)>(
        "SELECT driver_id, seed FROM tournament_registrations
         WHERE tournament_id = ? AND status IN ('registered', 'checked_in')
         ORDER BY COALESCE(seed, 9999), created_at",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    let regs = match regs {
        Ok(r) => r,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    if regs.len() < 2 {
        return Json(json!({ "error": "Need at least 2 registrations" }));
    }

    // Delete existing matches
    let _ = sqlx::query("DELETE FROM tournament_matches WHERE tournament_id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    // Generate round 1 matches (pair sequential registrations)
    let mut match_count = 0;
    let mut i = 0;
    while i < regs.len() {
        let driver_a = &regs[i].0;
        let driver_b = if i + 1 < regs.len() {
            Some(&regs[i + 1].0)
        } else {
            None // Bye
        };

        match_count += 1;
        let match_id = uuid::Uuid::new_v4().to_string();
        let _ = sqlx::query(
            "INSERT INTO tournament_matches (id, tournament_id, round, match_number, driver_a, driver_b, status)
             VALUES (?, ?, 1, ?, ?, ?, ?)",
        )
        .bind(&match_id)
        .bind(&id)
        .bind(match_count as i64)
        .bind(driver_a)
        .bind(driver_b)
        .bind(if driver_b.is_some() { "pending" } else { "completed" })
        .execute(&state.db)
        .await;

        // Auto-advance bye
        if driver_b.is_none() {
            let _ = sqlx::query(
                "UPDATE tournament_matches SET winner_id = ?, status = 'completed' WHERE id = ?",
            )
            .bind(driver_a)
            .bind(&match_id)
            .execute(&state.db)
            .await;
        }

        i += 2;
    }

    // Update tournament status
    let _ = sqlx::query("UPDATE tournaments SET status = 'in_progress' WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    Json(json!({ "ok": true, "round_1_matches": match_count }))
}

async fn record_match_result(
    State(state): State<Arc<AppState>>,
    Path((tournament_id, match_id)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let winner_id = match body.get("winner_id").and_then(|v| v.as_str()) {
        Some(w) => w,
        None => return Json(json!({ "error": "winner_id required" })),
    };

    let _ = sqlx::query(
        "UPDATE tournament_matches SET
            winner_id = ?, status = 'completed', completed_at = datetime('now'),
            time_a_ms = ?, time_b_ms = ?
         WHERE id = ? AND tournament_id = ?",
    )
    .bind(winner_id)
    .bind(body.get("time_a_ms").and_then(|v| v.as_i64()))
    .bind(body.get("time_b_ms").and_then(|v| v.as_i64()))
    .bind(&match_id)
    .bind(&tournament_id)
    .execute(&state.db)
    .await;

    // Check if all matches in current round are done, generate next round
    let current_round: Option<(i64,)> = sqlx::query_as(
        "SELECT round FROM tournament_matches WHERE id = ?",
    )
    .bind(&match_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((round,)) = current_round {
        let pending: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM tournament_matches
             WHERE tournament_id = ? AND round = ? AND status != 'completed'",
        )
        .bind(&tournament_id)
        .bind(round)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if pending.map(|p| p.0 == 0).unwrap_or(false) {
            // All done in this round — get winners and create next round
            let winners = sqlx::query_as::<_, (String,)>(
                "SELECT winner_id FROM tournament_matches
                 WHERE tournament_id = ? AND round = ? AND winner_id IS NOT NULL
                 ORDER BY match_number",
            )
            .bind(&tournament_id)
            .bind(round)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

            if winners.len() > 1 {
                let next_round = round + 1;
                let mut match_num = 0;
                let mut i = 0;
                while i < winners.len() {
                    match_num += 1;
                    let driver_a = &winners[i].0;
                    let driver_b = if i + 1 < winners.len() {
                        Some(&winners[i + 1].0)
                    } else {
                        None
                    };

                    let mid = uuid::Uuid::new_v4().to_string();
                    let _ = sqlx::query(
                        "INSERT INTO tournament_matches (id, tournament_id, round, match_number, driver_a, driver_b, status)
                         VALUES (?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&mid)
                    .bind(&tournament_id)
                    .bind(next_round)
                    .bind(match_num as i64)
                    .bind(driver_a)
                    .bind(driver_b)
                    .bind(if driver_b.is_some() { "pending" } else { "completed" })
                    .execute(&state.db)
                    .await;

                    if driver_b.is_none() {
                        let _ = sqlx::query(
                            "UPDATE tournament_matches SET winner_id = ?, status = 'completed' WHERE id = ?",
                        )
                        .bind(driver_a)
                        .bind(&mid)
                        .execute(&state.db)
                        .await;
                    }
                    i += 2;
                }
            } else if winners.len() == 1 {
                // Tournament complete!
                let _ = sqlx::query("UPDATE tournaments SET status = 'completed' WHERE id = ?")
                    .bind(&tournament_id)
                    .execute(&state.db)
                    .await;
                let _ = sqlx::query(
                    "UPDATE tournament_registrations SET status = 'winner' WHERE tournament_id = ? AND driver_id = ?",
                )
                .bind(&tournament_id)
                .bind(&winners[0].0)
                .execute(&state.db)
                .await;
            }
        }
    }

    Json(json!({ "ok": true }))
}

// ─── Customer Tournament Endpoints ──────────────────────────────────────────

async fn customer_list_tournaments(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i64, i64, i64, String, Option<String>)>(
        "SELECT id, name, description, track, car, format, max_participants,
                entry_fee_paise, prize_pool_paise, status, event_date
         FROM tournaments
         WHERE status IN ('upcoming', 'registration', 'in_progress')
         ORDER BY event_date ASC",
    )
    .fetch_all(&state.db)
    .await;

    let tournaments = match rows {
        Ok(t) => t,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Check which the driver is registered for
    let registered: Vec<String> = sqlx::query_as::<_, (String,)>(
        "SELECT tournament_id FROM tournament_registrations WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.0)
    .collect();

    let list: Vec<Value> = tournaments.iter().map(|t| {
        json!({
            "id": t.0, "name": t.1, "description": t.2,
            "track": t.3, "car": t.4, "format": t.5,
            "max_participants": t.6,
            "entry_fee_display": if t.7 > 0 { format!("Rs.{}", t.7 / 100) } else { "Free".to_string() },
            "prize_pool_display": if t.8 > 0 { format!("Rs.{}", t.8 / 100) } else { "TBD".to_string() },
            "status": t.9, "event_date": t.10,
            "is_registered": registered.contains(&t.0),
        })
    }).collect();

    Json(json!({ "tournaments": list }))
}

async fn customer_register_tournament(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Check tournament exists and is open
    let status: Option<(String, i64)> = sqlx::query_as(
        "SELECT status, max_participants FROM tournaments WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match &status {
        Some((s, _)) if s != "registration" && s != "upcoming" => {
            return Json(json!({ "error": "Registration is not open" }));
        }
        None => return Json(json!({ "error": "Tournament not found" })),
        _ => {}
    }

    let max = status.unwrap().1;

    // Check capacity
    let count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM tournament_registrations WHERE tournament_id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    if count >= max {
        return Json(json!({ "error": "Tournament is full" }));
    }

    let reg_id = uuid::Uuid::new_v4().to_string();
    let result = sqlx::query(
        "INSERT INTO tournament_registrations (id, tournament_id, driver_id) VALUES (?, ?, ?)",
    )
    .bind(&reg_id)
    .bind(&id)
    .bind(&driver_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true, "registration_id": reg_id })),
        Err(e) if e.to_string().contains("UNIQUE") => {
            Json(json!({ "error": "Already registered" }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Coaching: Lap Comparison ────────────────────────────────────────────────

#[derive(Deserialize)]
struct CompareLapsQuery {
    track: String,
    car: String,
    compare_to: Option<String>, // "record" or driver_id
}

async fn customer_compare_laps(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<CompareLapsQuery>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Get driver's laps on this track+car
    let my_laps = sqlx::query_as::<_, (i64, i64, Option<i64>, Option<i64>, Option<i64>, bool)>(
        "SELECT lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid
         FROM laps WHERE driver_id = ? AND track = ? AND car = ? AND valid = 1
         ORDER BY lap_time_ms ASC LIMIT 20",
    )
    .bind(&driver_id)
    .bind(&params.track)
    .bind(&params.car)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if my_laps.is_empty() {
        return Json(json!({ "error": "No laps found on this track/car" }));
    }

    let my_best = &my_laps[0];

    // Get comparison target
    let compare_to = params.compare_to.as_deref().unwrap_or("record");

    let reference_lap: Option<(String, i64, Option<i64>, Option<i64>, Option<i64>)> = if compare_to == "record" {
        // Compare to track record
        sqlx::query_as(
            "SELECT d.name, tr.best_lap_ms, l.sector1_ms, l.sector2_ms, l.sector3_ms
             FROM track_records tr
             JOIN drivers d ON tr.driver_id = d.id
             LEFT JOIN laps l ON tr.lap_id = l.id
             WHERE tr.track = ? AND tr.car = ?",
        )
        .bind(&params.track)
        .bind(&params.car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        // Compare to specific driver's best
        sqlx::query_as(
            "SELECT d.name, pb.best_lap_ms, l.sector1_ms, l.sector2_ms, l.sector3_ms
             FROM personal_bests pb
             JOIN drivers d ON pb.driver_id = d.id
             LEFT JOIN laps l ON pb.lap_id = l.id
             WHERE pb.driver_id = ? AND pb.track = ? AND pb.car = ?",
        )
        .bind(compare_to)
        .bind(&params.track)
        .bind(&params.car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    };

    // Compute sector deltas
    let sector_analysis = if let Some(ref_lap) = &reference_lap {
        let s1_delta = match (my_best.2, ref_lap.2) {
            (Some(mine), Some(theirs)) => Some(mine - theirs),
            _ => None,
        };
        let s2_delta = match (my_best.3, ref_lap.3) {
            (Some(mine), Some(theirs)) => Some(mine - theirs),
            _ => None,
        };
        let s3_delta = match (my_best.4, ref_lap.4) {
            (Some(mine), Some(theirs)) => Some(mine - theirs),
            _ => None,
        };

        let weakest = [
            s1_delta.map(|d| ("S1", d)),
            s2_delta.map(|d| ("S2", d)),
            s3_delta.map(|d| ("S3", d)),
        ]
        .iter()
        .filter_map(|x| *x)
        .max_by_key(|(_, d)| *d);

        Some(json!({
            "s1_delta_ms": s1_delta,
            "s2_delta_ms": s2_delta,
            "s3_delta_ms": s3_delta,
            "weakest_sector": weakest.map(|(s, d)| format!("{} (+{}ms)", s, d)),
            "total_delta_ms": my_best.1 - ref_lap.1,
        }))
    } else {
        None
    };

    // Consistency trend (last 10 laps chronologically)
    let recent_laps = sqlx::query_as::<_, (i64,)>(
        "SELECT lap_time_ms FROM laps
         WHERE driver_id = ? AND track = ? AND car = ? AND valid = 1
         ORDER BY created_at DESC LIMIT 10",
    )
    .bind(&driver_id)
    .bind(&params.track)
    .bind(&params.car)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let trend: Vec<i64> = recent_laps.iter().rev().map(|l| l.0).collect();
    let improving = if trend.len() >= 3 {
        let first_half: f64 = trend[..trend.len()/2].iter().map(|&t| t as f64).sum::<f64>() / (trend.len()/2) as f64;
        let second_half: f64 = trend[trend.len()/2..].iter().map(|&t| t as f64).sum::<f64>() / (trend.len() - trend.len()/2) as f64;
        Some(second_half < first_half)
    } else {
        None
    };

    Json(json!({
        "track": params.track,
        "car": params.car,
        "my_best": {
            "time_ms": my_best.1,
            "s1_ms": my_best.2,
            "s2_ms": my_best.3,
            "s3_ms": my_best.4,
        },
        "reference": reference_lap.as_ref().map(|r| json!({
            "driver": r.0,
            "time_ms": r.1,
            "s1_ms": r.2,
            "s2_ms": r.3,
            "s3_ms": r.4,
        })),
        "sector_analysis": sector_analysis,
        "recent_trend": trend,
        "improving": improving,
        "tip": sector_analysis.as_ref().and_then(|sa| {
            sa.get("weakest_sector").and_then(|w| w.as_str()).map(|w| {
                format!("Focus on {} — that is where you lose the most time vs the reference lap.", w)
            })
        }),
    }))
}

// ─── Bot endpoints (WhatsApp bot, terminal_secret auth) ─────────────────────

fn validate_bot_secret(state: &AppState, headers: &axum::http::HeaderMap) -> Result<(), Json<Value>> {
    let secret = state.config.cloud.terminal_secret.as_deref()
        .ok_or_else(|| Json(json!({ "error": "Terminal secret not configured" })))?;
    let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
    if provided != Some(secret) {
        return Err(Json(json!({ "error": "Unauthorized" })));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct BotLookupQuery {
    phone: String,
}

async fn bot_lookup(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<BotLookupQuery>,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    let phone = params.phone.trim();
    if phone.is_empty() {
        return Json(json!({ "error": "Phone number required" }));
    }

    // Look up driver by phone
    let driver = sqlx::query_as::<_, (String, String, Option<String>, bool)>(
        "SELECT id, name, phone, COALESCE(has_used_trial, 0) FROM drivers WHERE phone = ?",
    )
    .bind(phone)
    .fetch_optional(&state.db)
    .await;

    match driver {
        Ok(Some((id, name, _phone, has_used_trial))) => {
            // Get wallet balance
            let balance = wallet::get_balance(&state, &id).await.unwrap_or(0);

            Json(json!({
                "registered": true,
                "driver_id": id,
                "name": name,
                "wallet_balance_paise": balance,
                "has_used_trial": has_used_trial,
            }))
        }
        Ok(None) => Json(json!({
            "registered": false,
            "message": "No account found for this phone number",
        })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn bot_pricing(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    let rows = sqlx::query_as::<_, (String, String, i64, i64, bool, i64)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial, sort_order
         FROM pricing_tiers WHERE is_active = 1 ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(tiers) => {
            let list: Vec<Value> = tiers
                .iter()
                .map(|t| {
                    json!({
                        "id": t.0, "name": t.1, "duration_minutes": t.2,
                        "price_paise": t.3, "is_trial": t.4,
                    })
                })
                .collect();
            Json(json!({ "tiers": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

#[derive(Debug, Deserialize)]
struct BotBookRequest {
    phone: String,
    pricing_tier_id: String,
    experience_id: Option<String>,
}

async fn bot_book(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<BotBookRequest>,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    // Look up driver by phone
    let driver = sqlx::query_as::<_, (String, String, bool)>(
        "SELECT id, name, COALESCE(has_used_trial, 0) FROM drivers WHERE phone = ?",
    )
    .bind(&req.phone)
    .fetch_optional(&state.db)
    .await;

    let (driver_id, driver_name, has_used_trial) = match driver {
        Ok(Some(d)) => d,
        Ok(None) => return Json(json!({
            "status": "error",
            "error": "not_registered",
            "message": "No account found for this phone number. Please register at app.racingpoint.cloud first.",
        })),
        Err(e) => return Json(json!({ "status": "error", "error": format!("DB error: {}", e) })),
    };

    // Validate pricing tier
    let tier = match sqlx::query_as::<_, (String, String, i64, i64, bool)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&req.pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return Json(json!({ "status": "error", "error": "invalid_tier", "message": "Invalid pricing tier" })),
        Err(e) => return Json(json!({ "status": "error", "error": format!("DB error: {}", e) })),
    };

    let is_trial = tier.4;
    let price_paise = tier.3;
    let duration_minutes = tier.2;

    // Trial check
    if is_trial && has_used_trial {
        return Json(json!({
            "status": "error",
            "error": "trial_used",
            "message": "You've already used your free trial.",
        }));
    }

    // Wallet balance check for non-trial
    if !is_trial {
        let balance = match wallet::get_balance(&state, &driver_id).await {
            Ok(b) => b,
            Err(e) => return Json(json!({ "status": "error", "error": e })),
        };

        if balance < price_paise {
            return Json(json!({
                "status": "error",
                "error": "insufficient_balance",
                "message": format!("Insufficient balance. You have ₹{} but need ₹{}.", balance / 100, price_paise / 100),
                "balance_paise": balance,
                "required_paise": price_paise,
            }));
        }
    }

    // Check for existing active reservation
    if let Some(existing) = pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await {
        return Json(json!({
            "status": "error",
            "error": "active_reservation",
            "message": "You already have an active reservation.",
            "reservation_id": existing.id,
        }));
    }

    // Find idle pod
    let pod_id = match pod_reservation::find_idle_pod(&state).await {
        Some(id) => id,
        None => return Json(json!({
            "status": "error",
            "error": "no_pods",
            "message": "No pods available right now. Please try again shortly or visit us to get in the queue.",
        })),
    };

    let pod_number = {
        let pods = state.pods.read().await;
        pods.get(&pod_id).map(|p| p.number).unwrap_or(0)
    };

    // Debit wallet (skip for trial)
    let (wallet_txn_id, wallet_debit) = if !is_trial && price_paise > 0 {
        match wallet::debit(
            &state,
            &driver_id,
            price_paise,
            "debit_session",
            None,
            Some(&format!("{} on Pod {} (WhatsApp)", tier.1, pod_number)),
        )
        .await
        {
            Ok((_, txn_id)) => (Some(txn_id), Some(price_paise)),
            Err(e) => return Json(json!({ "status": "error", "error": e })),
        }
    } else {
        (None, None)
    };

    // Create pod reservation
    let reservation_id = match pod_reservation::create_reservation(&state, &driver_id, &pod_id).await {
        Ok(id) => id,
        Err(e) => {
            if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
                let _ = wallet::refund(&state, &driver_id, amount, None, Some("Bot booking failed — auto-refund")).await;
            }
            return Json(json!({ "status": "error", "error": e }));
        }
    };

    // Create auth token (PIN type)
    let experience_id = req.experience_id.clone();
    let auth_token = match auth::create_auth_token(
        &state,
        pod_id.clone(),
        driver_id.clone(),
        req.pricing_tier_id.clone(),
        "pin".to_string(),
        None,
        None,
        experience_id,
        None,
    )
    .await
    {
        Ok(token_info) => token_info,
        Err(e) => {
            let _ = pod_reservation::end_reservation(&state, &reservation_id).await;
            if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
                let _ = wallet::refund(&state, &driver_id, amount, None, Some("Bot booking failed — auto-refund")).await;
            }
            return Json(json!({ "status": "error", "error": format!("Failed to create auth: {}", e) }));
        }
    };

    Json(json!({
        "status": "booked",
        "booking_id": reservation_id,
        "driver_name": driver_name,
        "pod_number": pod_number,
        "pin": auth_token.token,
        "allocated_seconds": auth_token.allocated_seconds,
        "duration_minutes": duration_minutes,
        "tier_name": tier.1,
        "wallet_debit_paise": wallet_debit,
        "message": format!(
            "Session booked! Head to Pod {} and enter PIN {} on the screen. You have {} minutes.",
            pod_number, auth_token.token, duration_minutes
        ),
    }))
}
