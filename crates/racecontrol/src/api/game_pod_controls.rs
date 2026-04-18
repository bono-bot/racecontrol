#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage};

pub(crate) async fn set_pod_transmission(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let transmission = body
        .get("transmission")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");

    let sender = {
        let senders = state.agent_senders.read().await;
        senders.get(&pod_id).cloned()
    };
    if let Some(tx) = sender {
        let msg = CoreToAgentMessage::SetTransmission {
            transmission: transmission.to_string(),
        };
        if let Err(e) = tx.send(CoreMessage::wrap(msg)).await {
            tracing::error!("Failed to send SetTransmission to {}: {}", pod_id, e);
            return Json(json!({ "error": "Failed to send to agent" }));
        }
        tracing::info!("Set transmission to '{}' on pod {}", transmission, pod_id);
        Json(json!({ "ok": true, "transmission": transmission }))
    } else {
        Json(json!({ "error": "No agent connected for this pod" }))
    }
}

pub(crate) async fn set_pod_ffb(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    // Try numeric percent first (Phase 6 mid-session FFB gain)
    if let Some(percent) = body.get("percent").and_then(|v| v.as_u64()) {
        let percent = (percent as u8).clamp(10, 100);
        let sender = {
            let senders = state.agent_senders.read().await;
            senders.get(&pod_id).cloned()
        };
        if let Some(tx) = sender {
            let msg = CoreToAgentMessage::SetFfbGain { percent };
            if let Err(e) = tx.send(CoreMessage::wrap(msg)).await {
                tracing::error!("Failed to send SetFfbGain to {}: {}", pod_id, e);
                return Json(json!({ "error": "Failed to send to agent" }));
            }
            tracing::info!("Set FFB gain to {}% on pod {}", percent, pod_id);
            return Json(json!({ "ok": true, "ffb_percent": percent }));
        } else {
            return Json(json!({ "error": "No agent connected for this pod" }));
        }
    }

    // Legacy preset path (existing behavior)
    let preset = body
        .get("preset")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");

    let sender = {
        let senders = state.agent_senders.read().await;
        senders.get(&pod_id).cloned()
    };
    if let Some(tx) = sender {
        let msg = CoreToAgentMessage::SetFfb {
            preset: preset.to_string(),
        };
        if let Err(e) = tx.send(CoreMessage::wrap(msg)).await {
            tracing::error!("Failed to send SetFfb to {}: {}", pod_id, e);
            return Json(json!({ "error": "Failed to send to agent" }));
        }
        tracing::info!("Set FFB to '{}' on pod {}", preset, pod_id);
        Json(json!({ "ok": true, "preset": preset }))
    } else {
        Json(json!({ "error": "No agent connected for this pod" }))
    }
}

pub(crate) async fn set_pod_assists(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let assist_type = body.get("assist_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let enabled = body.get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Validate assist_type is one of the supported types
    // Stability control intentionally excluded per user decision (no runtime mechanism in AC)
    if !["abs", "tc", "transmission"].contains(&assist_type) {
        return Json(json!({ "error": "Invalid assist_type. Supported: abs, tc, transmission" }));
    }

    let sender = {
        let senders = state.agent_senders.read().await;
        senders.get(&pod_id).cloned()
    };
    if let Some(tx) = sender {
        let msg = CoreToAgentMessage::SetAssist {
            assist_type: assist_type.to_string(),
            enabled,
        };
        if let Err(e) = tx.send(CoreMessage::wrap(msg)).await {
            tracing::error!("Failed to send SetAssist to {}: {}", pod_id, e);
            return Json(json!({ "error": format!("Failed to send to agent: {}", e) }));
        }
        tracing::info!("Set assist {} = {} on pod {}", assist_type, enabled, pod_id);
        Json(json!({ "ok": true, "assist_type": assist_type, "enabled": enabled }))
    } else {
        Json(json!({ "error": "No agent connected for this pod" }))
    }
}

pub(crate) async fn get_pod_assist_state(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    // Read cached state immediately
    let cached = {
        let cache = state.assist_cache.read().await;
        cache.get(&pod_id).cloned()
    };

    // Also send QueryAssistState to agent for background refresh
    // (next time PWA opens the drawer, cache will be even fresher)
    let sender = {
        let senders = state.agent_senders.read().await;
        senders.get(&pod_id).cloned()
    };
    if let Some(tx) = sender
        && let Err(e) = tx.send(CoreMessage::wrap(CoreToAgentMessage::QueryAssistState)).await {
            tracing::warn!("Failed to send QueryAssistState to {}: {}", pod_id, e);
        }

    match cached {
        Some(s) => Json(json!({
            "ok": true,
            "abs": s.abs,
            "tc": s.tc,
            "auto_shifter": s.auto_shifter,
            "ffb_percent": s.ffb_percent,
        })),
        None => {
            // No cached state yet (pod never reported state).
            // Return defaults -- the background QueryAssistState will populate the cache.
            Json(json!({
                "ok": true,
                "abs": 0,
                "tc": 0,
                "auto_shifter": true,
                "ffb_percent": 70,
            }))
        }
    }
}
