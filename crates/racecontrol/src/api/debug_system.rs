#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// ─── Re-exports from split modules ──────────────────────────────────────
// Maintains API surface so `use super::debug_system::*` in routes.rs keeps working.
pub(crate) use crate::api::debug_incidents::*;
pub(crate) use crate::api::debug_fixes::*;

// ─── Debug System — Pod Events & Activity ───────────────────────────────

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
