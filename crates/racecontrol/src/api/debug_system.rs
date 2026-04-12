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

// ─── Debug System ────────────────────────────────────────────────────────

/// GET /debug/pod-events/{pod_id} — proxy recent diagnostic events from a pod's tier engine.
/// v27.0: Kiosk debug page fetches this to show recent autonomous + staff-triggered diagnostics.
pub(crate) async fn debug_pod_events(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
    Query(q): Query<PodEventsQuery>,
) -> Json<Value> {
    let limit = q.limit.unwrap_or(10).min(50);

    // P2 fix: Validate pod_id against known registered pods only.
    // The HashMap lookup IS the validation — unknown IDs get 404.
    // Additional format check prevents abuse (SSRF, log injection).

    // Look up the pod's IP address from the in-memory pod registry (not SQL)
    let pods = state.pods.read().await;
    let pod = pods.get(&pod_id).cloned();
    drop(pods);

    let Some(pod) = pod else {
        return Json(json!({ "events": [], "error": format!("Pod {} not found", pod_id) }));
    };

    // Fetch from pod's /events/recent endpoint
    let url = format!("http://{}:8090/events/recent?limit={}", pod.ip_address, limit);
    match state.http_client.get(&url).timeout(std::time::Duration::from_secs(5)).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<Value>().await {
                Ok(data) => Json(data),
                Err(e) => Json(json!({ "events": [], "error": format!("Parse error: {}", e) })),
            }
        }
        Ok(resp) => Json(json!({ "events": [], "error": format!("Pod returned {}", resp.status()) })),
        Err(e) => Json(json!({ "events": [], "error": format!("Pod unreachable: {}", e) })),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct PodEventsQuery {
    limit: Option<u32>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct DebugActivityQuery {
    hours: Option<f64>,
}

/// Track consecutive try_read() failures for starvation detection (MMA Round 3 P3).
pub(crate) static DEBUG_CONTENTION_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) async fn debug_activity(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DebugActivityQuery>,
) -> Json<Value> {
    let hours = q.hours.unwrap_or(2.0);
    let minutes = (hours * 60.0) as i64;
    let db = &state.db;

    // Pod health from in-memory state — use try_read() (non-blocking) to avoid deadlock.
    // 20+ write lock sites in billing/WS handlers can block readers indefinitely.
    // try_read() never queues — returns immediately with Err if lock is held.
    let (pod_health, pods_contended) = match state.pods.try_read() {
        Ok(pods) => {
            DEBUG_CONTENTION_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
            let now = chrono::Utc::now();
            let health: Vec<Value> = pods.values().map(|p| {
                let secs = p.last_seen
                    .map(|ls| (now - ls).num_seconds())
                    .unwrap_or(9999);
                let color = if secs > 9998 { "grey" }
                    else if secs > 15 { "red" }
                    else if secs > 10 { "orange" }
                    else if secs > 5 { "yellow" }
                    else { "green" };
                json!({
                    "pod_id": p.id,
                    "pod_number": p.number,
                    "seconds_since_heartbeat": secs,
                    "health": color,
                    "status": format!("{:?}", p.status),
                })
            }).collect();
            drop(pods);
            (health, false)
        }
        Err(_) => {
            let count = DEBUG_CONTENTION_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if count >= 5 {
                tracing::error!(target: "debug", consecutive = count, "debug_activity: pods RwLock STARVED — {} consecutive failures, possible write-lock monopoly", count);
            } else {
                tracing::warn!(target: "debug", consecutive = count, "debug_activity: pods RwLock contended");
            }
            (vec![], true)
        }
    };

    // Billing events — timeout DB queries to prevent indefinite hangs on SQLite lock contention
    let billing_json: Vec<Value> = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            "SELECT id, session_id, event_type, created_at, COALESCE(json_extract(details, '$.pod_id'), '') \
             FROM billing_events \
             WHERE created_at > datetime('now', ? || ' minutes') \
             ORDER BY created_at DESC LIMIT 200",
        )
        .bind(format!("-{}", minutes))
        .fetch_all(db),
    ).await {
        Ok(Ok(events)) => events.iter().map(|(id, sid, etype, ts, pod)| {
            json!({ "id": id, "session_id": sid, "event_type": etype, "created_at": ts, "pod_id": pod })
        }).collect(),
        Ok(Err(e)) => {
            tracing::warn!(target: "debug", "debug_activity: billing query error: {}", e);
            vec![]
        }
        Err(_) => {
            tracing::warn!(target: "debug", "debug_activity: billing query timeout (5s)");
            vec![]
        }
    };

    // Game launch events
    let game_json: Vec<Value> = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            "SELECT id, pod_id, event_type, created_at, COALESCE(error_message, '') \
             FROM game_launch_events \
             WHERE created_at > datetime('now', ? || ' minutes') \
             ORDER BY created_at DESC LIMIT 200",
        )
        .bind(format!("-{}", minutes))
        .fetch_all(db),
    ).await {
        Ok(Ok(events)) => events.iter().map(|(id, pod, etype, ts, err)| {
            json!({ "id": id, "pod_id": pod, "event_type": etype, "created_at": ts, "error_message": err })
        }).collect(),
        Ok(Err(e)) => {
            tracing::warn!(target: "debug", "debug_activity: game events query error: {}", e);
            vec![]
        }
        Err(_) => {
            tracing::warn!(target: "debug", "debug_activity: game events query timeout (5s)");
            vec![]
        }
    };

    // Include contention flag so kiosk UI can show "data temporarily unavailable" instead of "all pods down"
    Json(json!({
        "pod_health": pod_health,
        "billing_events": billing_json,
        "game_events": game_json,
        "pods_contended": pods_contended,
    }))
}

pub(crate) async fn debug_playbooks(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let playbooks: Vec<Value> = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT id, category, title, steps FROM debug_playbooks ORDER BY category",
        )
        .fetch_all(&state.db),
    ).await {
        Ok(Ok(rows)) => rows.iter().map(|(id, cat, title, steps)| {
            let parsed: Value = serde_json::from_str(steps).unwrap_or(json!([]));
            json!({ "id": id, "category": cat, "title": title, "steps": parsed })
        }).collect(),
        Ok(Err(e)) => {
            tracing::warn!(target: "debug", "debug_playbooks query error: {}", e);
            vec![]
        }
        Err(_) => {
            tracing::warn!(target: "debug", "debug_playbooks: DB query timeout (5s)");
            vec![]
        }
    };

    Json(json!({ "playbooks": playbooks }))
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct CreateIncidentBody {
    description: String,
    pod_id: Option<String>,
}

pub(crate) async fn create_debug_incident(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateIncidentBody>,
) -> Json<Value> {
    let db = &state.db;
    let desc_lower = body.description.to_lowercase();

    // Auto-detect category
    let category = if desc_lower.contains("offline") || desc_lower.contains("down") || desc_lower.contains("not working") || desc_lower.contains("dead") {
        "pod_offline"
    } else if desc_lower.contains("crash") || desc_lower.contains("won't launch") || desc_lower.contains("game error") || desc_lower.contains("wont launch") {
        "game_crash"
    } else if desc_lower.contains("billing") || desc_lower.contains("timer") || desc_lower.contains("session stuck") {
        "billing_stuck"
    } else if desc_lower.contains("blank") || desc_lower.contains("screen stuck") || desc_lower.contains("lock screen") {
        "screen_stuck"
    } else if desc_lower.contains("steering") || desc_lower.contains("pedal") || desc_lower.contains("wheel") || desc_lower.contains("input") {
        "no_steering_input"
    } else if desc_lower.contains("idle") || desc_lower.contains("not counting") || desc_lower.contains("pausing") {
        "high_idle_time"
    } else if desc_lower.contains("sync") || desc_lower.contains("cloud") || desc_lower.contains("not updating") {
        "sync_failure"
    } else if desc_lower.contains("kiosk") || desc_lower.contains("bypass") || desc_lower.contains("desktop") || desc_lower.contains("taskbar") {
        "kiosk_bypass"
    } else {
        "unknown"
    };

    // Find matching playbook
    let playbook = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, category, title, steps FROM debug_playbooks WHERE category = ?",
    )
    .bind(category)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let playbook_id = playbook.as_ref().map(|p| p.0.clone());

    // Capture context snapshot
    let pods = state.pods.read().await;
    let active_sessions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions WHERE status = 'active'",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let pod_snapshot = if let Some(ref pid) = body.pod_id {
        pods.get(pid).map(|p| json!({
            "status": format!("{:?}", p.status),
            "last_seen": p.last_seen,
            "driving_state": p.driving_state,
            "current_game": p.sim_type,
        }))
    } else {
        None
    };
    drop(pods);

    let context = json!({
        "pod_state": pod_snapshot,
        "active_sessions": active_sessions,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    let id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO debug_incidents (id, pod_id, category, description, status, context_snapshot, playbook_id) \
         VALUES (?, ?, ?, ?, 'open', ?, ?)",
    )
    .bind(&id)
    .bind(&body.pod_id)
    .bind(category)
    .bind(&body.description)
    .bind(context.to_string())
    .bind(&playbook_id)
    .execute(db)
    .await;

    // Log to activity feed so staff messages appear in real-time
    let pod_id_for_log = body.pod_id.as_deref().unwrap_or("system");
    crate::activity_log::log_pod_activity(
        &state,
        pod_id_for_log,
        "system",
        "Staff Report",
        &body.description,
        "staff",
        None,
    );

    let playbook_json = playbook.map(|(pid, cat, title, steps)| {
        let parsed: Value = serde_json::from_str(&steps).unwrap_or(json!([]));
        json!({ "id": pid, "category": cat, "title": title, "steps": parsed })
    });

    // Suggest quick actions based on category
    let suggested_actions: Vec<&str> = match category {
        "pod_offline" => vec!["restart_pod", "wake_pod"],
        "game_crash" => vec!["kill_game"],
        "screen_stuck" => vec!["relaunch_edge"],
        "no_steering_input" => vec!["restart_pod"],
        "kiosk_bypass" => vec!["relaunch_edge"],
        "billing_stuck" | "high_idle_time" | "sync_failure" | "unknown" => vec![],
        _ => vec![],
    };

    // ─── v27.0: Send DiagnosticRequest to pod for Tier 1 + Tier 2 diagnosis ──
    // NOTE: Skip for "pod_offline" category — if the pod is truly offline, the WS send
    // will fail silently. The server's own AI diagnosis (Claude/Ollama) handles offline pods.
    // Use incident ID as correlation_id so the returning DiagnosticResult can be
    // directly linked to the incident in the DB (MMA R4-1 fix: broken correlation chain)
    let correlation_id = id.clone();
    let mut tier_diagnosis_sent = false;
    if category != "pod_offline" {
    if let Some(ref pid) = body.pod_id {
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pid) {
            let diag_req = CoreToAgentMessage::DiagnosticRequest {
                correlation_id: correlation_id.clone(),
                incident_id: id.clone(),
                description: body.description.clone(),
                category: category.to_string(),
                requested_by: "staff".to_string(),
            };
            if sender.send(CoreMessage::wrap(diag_req)).await.is_ok() {
                tier_diagnosis_sent = true;
                tracing::info!(
                    target: "debug-bridge",
                    pod = %pid,
                    correlation_id = %correlation_id,
                    "DiagnosticRequest sent to pod for incident {}",
                    id
                );
            }
        }
        drop(agent_senders);
    }
    } // end category != "pod_offline" guard

    Json(json!({
        "incident": {
            "id": id,
            "pod_id": body.pod_id,
            "category": category,
            "description": body.description,
            "status": "open",
            "playbook_id": playbook_id,
            "created_at": chrono::Utc::now().to_rfc3339(),
        },
        "playbook": playbook_json,
        "suggested_actions": suggested_actions,
        "tier_diagnosis": {
            "sent": tier_diagnosis_sent,
            "correlation_id": correlation_id,
        },
    }))
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct DebugIncidentFilter {
    status: Option<String>,
}

pub(crate) async fn list_debug_incidents(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DebugIncidentFilter>,
) -> Json<Value> {
    let db = &state.db;

    let rows = if let Some(ref status) = q.status {
        sqlx::query_as::<_, (String, Option<String>, String, String, String, Option<String>, String)>(
            "SELECT id, pod_id, category, description, status, playbook_id, created_at \
             FROM debug_incidents WHERE status = ? ORDER BY created_at DESC LIMIT 100",
        )
        .bind(status)
        .fetch_all(db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as::<_, (String, Option<String>, String, String, String, Option<String>, String)>(
            "SELECT id, pod_id, category, description, status, playbook_id, created_at \
             FROM debug_incidents ORDER BY created_at DESC LIMIT 100",
        )
        .fetch_all(db)
        .await
        .unwrap_or_default()
    };

    let incidents: Vec<Value> = rows.iter().map(|(id, pod, cat, desc, status, pb, ts)| {
        json!({
            "id": id, "pod_id": pod, "category": cat,
            "description": desc, "status": status,
            "playbook_id": pb, "created_at": ts,
        })
    }).collect();

    Json(json!({ "incidents": incidents }))
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct UpdateIncidentBody {
    status: Option<String>,
    resolution_text: Option<String>,
    effectiveness: Option<i32>,
}

pub(crate) async fn update_debug_incident(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateIncidentBody>,
) -> Json<Value> {
    let db = &state.db;

    if let Some(ref status) = body.status {
        let resolved_at = if status == "resolved" {
            Some(chrono::Utc::now().to_rfc3339())
        } else {
            None
        };
        let _ = sqlx::query(
            "UPDATE debug_incidents SET status = ?, resolved_at = COALESCE(?, resolved_at) WHERE id = ?",
        )
        .bind(status)
        .bind(&resolved_at)
        .bind(&id)
        .execute(db)
        .await;
    }

    // If resolving with text, save to RAG knowledge base
    if let Some(ref text) = body.resolution_text {
        let category: Option<String> = sqlx::query_scalar(
            "SELECT category FROM debug_incidents WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

        if let Some(cat) = category {
            let res_id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO debug_resolutions (id, incident_id, category, resolution_text, effectiveness) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&res_id)
            .bind(&id)
            .bind(&cat)
            .bind(text)
            .bind(body.effectiveness.unwrap_or(3))
            .execute(db)
            .await;
        }
    }

    Json(json!({ "ok": true, "id": id }))
}

// ─── POST /debug/incidents/{id}/apply-fix — Execute a quick fix action from debug page ──
#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct ApplyFixBody {
    /// One of: restart_pod, wake_pod, shutdown_pod, relaunch_edge, kill_game
    action: String,
    pod_id: Option<String>,
}

pub(crate) async fn debug_apply_fix(
    State(state): State<Arc<AppState>>,
    Path(incident_id): Path<String>,
    Json(body): Json<ApplyFixBody>,
) -> Json<Value> {
    let db = &state.db;

    // Verify incident exists
    let incident = sqlx::query_as::<_, (String, Option<String>, String)>(
        "SELECT id, pod_id, category FROM debug_incidents WHERE id = ?",
    )
    .bind(&incident_id)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let Some((inc_id, inc_pod_id, category)) = incident else {
        return Json(json!({ "ok": false, "error": "Incident not found" }));
    };

    // Resolve target pod — prefer explicit pod_id, fall back to incident's pod_id
    let target_pod_id = body.pod_id.or(inc_pod_id);
    let Some(ref pod_id) = target_pod_id else {
        return Json(json!({ "ok": false, "error": "No pod specified — select a pod first" }));
    };

    // Look up pod info
    let pods = state.pods.read().await;
    let pod = match pods.get(pod_id) {
        Some(p) => p.clone(),
        None => {
            drop(pods);
            return Json(json!({ "ok": false, "error": format!("Pod {} not found", pod_id) }));
        }
    };
    drop(pods);

    let action_label = body.action.clone();
    let result = match body.action.as_str() {
        "restart_pod" => {
            match wol::restart_pod(&state.http_client, &pod.ip_address).await {
                Ok(output) => json!({ "ok": true, "action": "restart_pod", "output": output }),
                Err(e) => json!({ "ok": false, "error": format!("Restart failed: {}", e) }),
            }
        }
        "wake_pod" => {
            if let Some(ref mac) = pod.mac_address {
                match wol::send_wol(mac).await {
                    Ok(_) => json!({ "ok": true, "action": "wake_pod" }),
                    Err(e) => json!({ "ok": false, "error": format!("WoL failed: {}", e) }),
                }
            } else {
                json!({ "ok": false, "error": format!("Pod {} has no MAC address configured", pod.number) })
            }
        }
        "shutdown_pod" => {
            match wol::shutdown_pod(&state.http_client, &pod.ip_address).await {
                Ok(output) => json!({ "ok": true, "action": "shutdown_pod", "output": output }),
                Err(e) => json!({ "ok": false, "error": format!("Shutdown failed: {}", e) }),
            }
        }
        "relaunch_edge" => {
            // Kill Edge and relaunch kiosk — executed via WS exec on the pod
            let cmd = "taskkill /F /IM msedge.exe & ping -n 3 127.0.0.1 >nul & start msedge.exe --kiosk http://localhost:3300 --edge-kiosk-type=fullscreen";
            match crate::ws::ws_exec_on_pod(&state, pod_id, cmd, 15_000).await {
                Ok((success, stdout, stderr)) => {
                    if success {
                        json!({ "ok": true, "action": "relaunch_edge", "output": stdout })
                    } else {
                        json!({ "ok": false, "error": format!("Edge relaunch failed: {}", stderr) })
                    }
                }
                Err(e) => json!({ "ok": false, "error": format!("Edge relaunch failed: {}", e) }),
            }
        }
        "kill_game" => {
            // Kill any running game process via WS exec
            let cmd = "taskkill /F /IM acs.exe & taskkill /F /IM acc.exe & taskkill /F /IM FormulaOne.exe";
            match crate::ws::ws_exec_on_pod(&state, pod_id, cmd, 10_000).await {
                Ok((success, stdout, stderr)) => {
                    if success {
                        json!({ "ok": true, "action": "kill_game", "output": stdout })
                    } else {
                        json!({ "ok": false, "error": format!("Kill game failed: {}", stderr) })
                    }
                }
                Err(e) => json!({ "ok": false, "error": format!("Kill game failed: {}", e) }),
            }
        }
        _ => json!({ "ok": false, "error": format!("Unknown action: {}", body.action) }),
    };

    let success = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);

    // Log to activity feed
    let detail = if success {
        format!("Applied fix '{}' on Pod {}", action_label, pod.number)
    } else {
        let err = result.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
        format!("Fix '{}' failed on Pod {}: {}", action_label, pod.number, err)
    };
    crate::activity_log::log_pod_activity(&state, pod_id, "race_engineer", "Quick Fix Applied", &detail, "staff", None);

    // v27.0: Notify pod's tier engine about the staff action to reset dedup window
    if success {
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pod_id) {
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::StaffActionNotify {
                action: action_label.clone(),
                reason: format!("Staff quick-fix for incident {}", incident_id),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            })).await;
        }
        drop(agent_senders);
    }

    // If action succeeded, auto-resolve the incident with the action as resolution
    if success {
        let resolved_at = chrono::Utc::now().to_rfc3339();
        let _ = sqlx::query(
            "UPDATE debug_incidents SET status = 'resolved', resolved_at = ? WHERE id = ? AND status = 'open'",
        )
        .bind(&resolved_at)
        .bind(&inc_id)
        .execute(db)
        .await;

        // Save to RAG knowledge base so future diagnosis can reference this fix
        let res_id = uuid::Uuid::new_v4().to_string();
        let resolution_text = format!("Quick fix: {} (applied from debug page)", action_label);
        let _ = sqlx::query(
            "INSERT INTO debug_resolutions (id, incident_id, category, resolution_text, effectiveness) \
             VALUES (?, ?, ?, ?, 4)",
        )
        .bind(&res_id)
        .bind(&inc_id)
        .bind(&category)
        .bind(&resolution_text)
        .execute(db)
        .await;
    }

    Json(result)
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct DiagnoseBody {
    incident_id: String,
}

pub(crate) async fn debug_diagnose(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DiagnoseBody>,
) -> Json<Value> {
    if !state.config.ai_debugger.enabled {
        return Json(json!({ "error": "AI debugger is not enabled" }));
    }

    let db = &state.db;

    // Load incident
    let incident = sqlx::query_as::<_, (String, Option<String>, String, String, Option<String>)>(
        "SELECT id, pod_id, category, description, context_snapshot FROM debug_incidents WHERE id = ?",
    )
    .bind(&body.incident_id)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let Some((inc_id, pod_id, category, description, ctx_snapshot)) = incident else {
        return Json(json!({ "error": "Incident not found" }));
    };

    // Load matching playbook
    let playbook = sqlx::query_as::<_, (String, String, String)>(
        "SELECT title, category, steps FROM debug_playbooks WHERE category = ?",
    )
    .bind(&category)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    // Load past resolutions for same category (RAG)
    let past_resolutions = sqlx::query_as::<_, (String, i32, String)>(
        "SELECT resolution_text, effectiveness, created_at FROM debug_resolutions \
         WHERE category = ? ORDER BY effectiveness DESC, created_at DESC LIMIT 5",
    )
    .bind(&category)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    // Build AI prompt
    let biz_context = crate::ai::gather_business_context(
        &state.db, &state.pods, &state.billing, &state.game_launcher,
    ).await;

    let mut prompt_parts = vec![
        format!("INCIDENT: {}", description),
        format!("CATEGORY: {}", category),
    ];

    if let Some(ref pid) = pod_id {
        prompt_parts.push(format!("POD: {}", pid));
    }
    if let Some(ref ctx) = ctx_snapshot {
        prompt_parts.push(format!("CONTEXT SNAPSHOT: {}", ctx));
    }
    if let Some(ref pb) = playbook {
        prompt_parts.push(format!("PLAYBOOK ({}): {}", pb.0, pb.2));
    }
    if !past_resolutions.is_empty() {
        let mut rag = String::from("PAST RESOLUTIONS FOR THIS CATEGORY:\n");
        for (text, eff, ts) in &past_resolutions {
            rag.push_str(&format!("  - [effectiveness={}/5, {}] {}\n", eff, ts, text));
        }
        prompt_parts.push(rag);
    }
    prompt_parts.push(format!("VENUE STATE:\n{}", biz_context));

    let messages = vec![
        json!({
            "role": "system",
            "content": "You are James, AI operations assistant for RacingPoint eSports venue. \
                        A staff member reported an incident. Analyze the issue using the playbook, \
                        past resolutions, and current venue state. Provide: 1) Root cause analysis, \
                        2) Step-by-step fix instructions, 3) Whether this matches a known pattern. \
                        Be concise and actionable."
        }),
        json!({
            "role": "user",
            "content": prompt_parts.join("\n\n")
        }),
    ];

    match crate::ai::query_ai(&state.config.ai_debugger, &messages, Some(db), Some("debug_incident")).await {
        Ok((diagnosis, model)) => {
            let playbook_json = playbook.map(|(title, cat, steps)| {
                let parsed: Value = serde_json::from_str(&steps).unwrap_or(json!([]));
                json!({ "category": cat, "title": title, "steps": parsed })
            });

            let past_json: Vec<Value> = past_resolutions.iter().map(|(text, eff, ts)| {
                json!({ "resolution_text": text, "effectiveness": eff, "created_at": ts })
            }).collect();

            // Log diagnosis to activity feed
            let detail = if diagnosis.len() > 120 { format!("{}...", &diagnosis[..120]) } else { diagnosis.clone() };
            let log_pod = pod_id.as_deref().unwrap_or("system");
            crate::activity_log::log_pod_activity(&state, log_pod, "race_engineer", "AI Diagnosis", &detail, "race_engineer", None);

            Json(json!({
                "diagnosis": diagnosis,
                "model": model,
                "incident_id": inc_id,
                "playbook": playbook_json,
                "past_resolutions": past_json,
            }))
        }
        Err(e) => {
            let log_pod = pod_id.as_deref().unwrap_or("system");
            crate::activity_log::log_pod_activity(&state, log_pod, "race_engineer", "AI Diagnosis Failed", &e.to_string(), "race_engineer", None);
            Json(json!({
                "error": format!("AI diagnosis failed: {}", e),
                "incident_id": inc_id,
            }))
        },
    }
}
