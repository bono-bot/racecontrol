#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;
use crate::wol;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage};

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

// ─── AI Diagnosis ───────────────────────────────────────────────────────

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
