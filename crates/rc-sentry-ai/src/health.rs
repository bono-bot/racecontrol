use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::routing::get;
use axum::Json;
use serde_json::{json, Value};

use crate::frame::FrameBuffer;
use crate::relay;

pub struct AppState {
    pub frame_buf: FrameBuffer,
    pub relay_api_url: String,
    pub start_time: Instant,
}

pub type SharedState = Arc<AppState>;

pub fn health_router(state: SharedState) -> axum::Router {
    axum::Router::new()
        .route("/health", get(health_handler))
        .route("/cameras", get(cameras_handler))
        .with_state(state)
}

fn camera_status_str(last_frame_secs_ago: f64) -> &'static str {
    if last_frame_secs_ago < 10.0 {
        "connected"
    } else if last_frame_secs_ago < 30.0 {
        "reconnecting"
    } else {
        "disconnected"
    }
}

async fn health_handler(State(state): State<SharedState>) -> Json<Value> {
    let cam_statuses = state.frame_buf.status().await;
    let relay_status = relay::check_relay_health(&state.relay_api_url).await;

    let uptime_secs = state.start_time.elapsed().as_secs();

    let mut cameras = serde_json::Map::new();
    let mut any_connected = false;

    for (name, cs) in &cam_statuses {
        let status_str = camera_status_str(cs.last_frame_secs_ago);
        if status_str == "connected" {
            any_connected = true;
        }
        cameras.insert(
            name.clone(),
            json!({
                "status": status_str,
                "last_frame_secs_ago": cs.last_frame_secs_ago,
                "frames_total": cs.frames_total,
            }),
        );
    }

    let overall_status = if relay_status.status != "healthy" {
        "error"
    } else if !any_connected && !cam_statuses.is_empty() {
        "degraded"
    } else if any_connected
        && cam_statuses
            .values()
            .any(|cs| camera_status_str(cs.last_frame_secs_ago) != "connected")
    {
        "degraded"
    } else {
        "ok"
    };

    Json(json!({
        "service": "rc-sentry-ai",
        "status": overall_status,
        "uptime_secs": uptime_secs,
        "cameras": cameras,
        "relay": {
            "status": relay_status.status,
            "api_url": relay_status.api_url,
        },
    }))
}

async fn cameras_handler(State(state): State<SharedState>) -> Json<Value> {
    let cam_statuses = state.frame_buf.status().await;

    let mut cameras = serde_json::Map::new();
    for (name, cs) in &cam_statuses {
        cameras.insert(
            name.clone(),
            json!({
                "status": camera_status_str(cs.last_frame_secs_ago),
                "last_frame_secs_ago": cs.last_frame_secs_ago,
                "frames_total": cs.frames_total,
            }),
        );
    }

    Json(json!({ "cameras": cameras }))
}
