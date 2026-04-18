#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage};

// ─── Playbooks ──────────────────────────────────────────────────────────

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

// ─── Incidents CRUD ─────────────────────────────────────────────────────

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

    // Auto-detect category — ordered most-specific first to avoid false matches
    let category = if desc_lower.contains("offline") || desc_lower.contains("down") || desc_lower.contains("not working") || desc_lower.contains("dead") {
        "pod_offline"
    } else if desc_lower.contains("frozen") || desc_lower.contains("not responding") || desc_lower.contains("hung") || desc_lower.contains("stuck game") || desc_lower.contains("game stuck") {
        "game_frozen"
    } else if desc_lower.contains("crash") || desc_lower.contains("won't launch") || desc_lower.contains("game error") || desc_lower.contains("wont launch")
        || desc_lower.contains("won't start") || desc_lower.contains("wont start") || desc_lower.contains("not starting") || desc_lower.contains("launch stuck")
        || desc_lower.contains("launch failed") || desc_lower.contains("loading forever") || desc_lower.contains("black screen")
        || desc_lower.contains("game black") || desc_lower.contains("game blank") {
        "game_crash"
    } else if desc_lower.contains("billing") || desc_lower.contains("timer") || desc_lower.contains("session stuck") {
        "billing_stuck"
    } else if desc_lower.contains("sound") || desc_lower.contains("audio") || desc_lower.contains("no sound") || desc_lower.contains("mute")
        || desc_lower.contains("volume") || desc_lower.contains("speaker") || desc_lower.contains("headphone") {
        "no_audio"
    } else if desc_lower.contains("blank") || desc_lower.contains("screen stuck") || desc_lower.contains("lock screen") {
        "screen_stuck"
    } else if desc_lower.contains("force feedback") || desc_lower.contains("ffb") || desc_lower.contains("no torque") || desc_lower.contains("wheel dead")
        || desc_lower.contains("steering") || desc_lower.contains("pedal") || desc_lower.contains("wheel") || desc_lower.contains("input") {
        "no_steering_input"
    } else if desc_lower.contains("slow") || desc_lower.contains("lag") || desc_lower.contains("fps") || desc_lower.contains("stuttering")
        || desc_lower.contains("choppy") || desc_lower.contains("frame rate") || desc_lower.contains("frame drop") || desc_lower.contains("performance") {
        "poor_performance"
    } else if desc_lower.contains("steam") || desc_lower.contains("steam update") || desc_lower.contains("steam login") || desc_lower.contains("steam popup") {
        "steam_blocked"
    } else if desc_lower.contains("werfault") || desc_lower.contains("error popup") || desc_lower.contains("error dialog") || desc_lower.contains("error message")
        || desc_lower.contains("popup") || desc_lower.contains("crash report") || desc_lower.contains("dialog box") {
        "error_dialog"
    } else if desc_lower.contains("idle") || desc_lower.contains("not counting") || desc_lower.contains("pausing") {
        "high_idle_time"
    } else if desc_lower.contains("sync") || desc_lower.contains("cloud") || desc_lower.contains("not updating") {
        "sync_failure"
    } else if desc_lower.contains("kiosk") || desc_lower.contains("bypass") || desc_lower.contains("desktop") || desc_lower.contains("taskbar") {
        "kiosk_bypass"
    } else if desc_lower.contains("missing track") || desc_lower.contains("missing car") || desc_lower.contains("content not found")
        || desc_lower.contains("dlc") || desc_lower.contains("content missing") || desc_lower.contains("track not found") || desc_lower.contains("car not found") {
        "content_missing"
    } else if desc_lower.contains("network") || desc_lower.contains("multiplayer") || desc_lower.contains("lobby") || desc_lower.contains("can't connect")
        || desc_lower.contains("cannot connect") || desc_lower.contains("connection") || desc_lower.contains("latency") || desc_lower.contains("ping") {
        "network_issue"
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
        "game_frozen" => vec!["kill_game"],
        "screen_stuck" => vec!["relaunch_edge"],
        "no_steering_input" => vec!["restart_pod"],
        "no_audio" => vec!["restart_audio"],
        "poor_performance" => vec!["restart_pod"],
        "steam_blocked" => vec!["restart_steam"],
        "error_dialog" => vec!["dismiss_dialogs"],
        "kiosk_bypass" => vec!["relaunch_edge"],
        "content_missing" | "network_issue" | "billing_stuck" | "high_idle_time" | "sync_failure" | "unknown" => vec![],
        _ => vec![],
    };

    // ─── v27.0: Send DiagnosticRequest to pod for Tier 1 + Tier 2 diagnosis ──
    // NOTE: Skip for "pod_offline" category — if the pod is truly offline, the WS send
    // will fail silently. The server's own AI diagnosis (Claude/Ollama) handles offline pods.
    // Use incident ID as correlation_id so the returning DiagnosticResult can be
    // directly linked to the incident in the DB (MMA R4-1 fix: broken correlation chain)
    let correlation_id = id.clone();
    let mut tier_diagnosis_sent = false;
    if category != "pod_offline"
    && let Some(ref pid) = body.pod_id {
        let sender = {
            let agent_senders = state.agent_senders.read().await;
            agent_senders.get(pid).cloned()
        };
        if let Some(sender) = sender {
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
    } // end category != "pod_offline" guard

    // ─── AUTO-FIX: If category has a suggested action AND pod is selected, apply immediately ──
    // Staff confirmed the issue by submitting — no second click needed.
    // Only auto-fix for categories with safe, deterministic first-choice actions.
    let mut auto_fix_result: Option<serde_json::Value> = None;
    if !suggested_actions.is_empty() {
        if let Some(ref pid) = body.pod_id {
            let pods_for_fix = state.pods.read().await;
            if let Some(pod) = pods_for_fix.get(pid) {
                let pod_clone = pod.clone();
                let pod_number = pod.number;
                drop(pods_for_fix);

                let action = suggested_actions[0]; // first suggested = highest priority
                tracing::info!(
                    target: "debug-bridge",
                    pod = %pid,
                    action = %action,
                    category = %category,
                    "Auto-applying fix for staff-reported incident {}",
                    id
                );

                let result = super::debug_fixes::execute_fix_action(&state, pid, &pod_clone, action).await;
                let success = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                let error_msg = result.get("error").and_then(|v| v.as_str()).map(|s| s.to_string());

                super::debug_fixes::post_fix_bookkeeping(
                    &state, pid, pod_number, &id, category, action,
                    success, error_msg.as_deref(), "Auto Fix",
                ).await;

                auto_fix_result = Some(result);
            } else {
                drop(pods_for_fix);
            }
        }
    }

    // Determine incident status — if auto-fix succeeded, it's already resolved
    let incident_status = if auto_fix_result
        .as_ref()
        .and_then(|r| r.get("ok"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        "resolved"
    } else {
        "open"
    };

    Json(json!({
        "incident": {
            "id": id,
            "pod_id": body.pod_id,
            "category": category,
            "description": body.description,
            "status": incident_status,
            "playbook_id": playbook_id,
            "created_at": chrono::Utc::now().to_rfc3339(),
        },
        "playbook": playbook_json,
        "suggested_actions": suggested_actions,
        "auto_fix": auto_fix_result,
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
