use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Json;
use futures::TryStreamExt;
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};

use crate::attendance;
use crate::config::CameraConfig;
use crate::nvr::NvrClient;

/// Shared state for NVR playback proxy endpoints.
pub struct PlaybackState {
    pub nvr_client: NvrClient,
    pub cameras: Vec<CameraConfig>,
    pub db_path: String,
}

/// Build the playback proxy router with CORS enabled.
pub fn playback_router(state: Arc<PlaybackState>) -> axum::Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET])
        .allow_headers(Any);

    axum::Router::new()
        .route("/api/v1/playback/search", get(search_handler))
        .route("/api/v1/playback/stream", get(stream_handler))
        .route("/api/v1/playback/events", get(events_handler))
        .with_state(state)
        .layer(cors)
}

// --- Search handler ---

#[derive(Deserialize)]
struct SearchParams {
    camera: String,
    start: String,
    end: String,
}

/// GET /api/v1/playback/search -- search NVR recordings by camera, date/time range.
async fn search_handler(
    State(state): State<Arc<PlaybackState>>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    // Look up nvr_channel from camera name
    let camera = match state.cameras.iter().find(|c| c.name == params.camera) {
        Some(cam) => cam,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("camera '{}' not found", params.camera)})),
            )
                .into_response();
        }
    };

    let channel = match camera.nvr_channel {
        Some(ch) => ch,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("camera '{}' has no nvr_channel configured", params.camera)})),
            )
                .into_response();
        }
    };

    match state.nvr_client.search_files(channel, &params.start, &params.end).await {
        Ok(files) => Json(json!(files)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("NVR search failed: {e}")})),
        )
            .into_response(),
    }
}

// --- Stream handler ---

#[derive(Deserialize)]
struct StreamParams {
    file_path: String,
}

/// GET /api/v1/playback/stream -- proxy recorded video from NVR.
async fn stream_handler(
    State(state): State<Arc<PlaybackState>>,
    Query(params): Query<StreamParams>,
) -> impl IntoResponse {
    let response = match state.nvr_client.stream_file(&params.file_path).await {
        Ok(resp) => resp,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("NVR stream failed: {e}")})),
            )
                .into_response();
        }
    };

    // Proxy the NVR response body as a byte stream without buffering
    let stream = response.bytes_stream().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("NVR stream error: {e}"))
    });

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::CONTENT_DISPOSITION, "inline")
        .header(header::CACHE_CONTROL, "no-cache, no-store")
        .body(Body::from_stream(stream))
        .expect("valid response")
        .into_response()
}

// --- Events handler ---

#[derive(Deserialize)]
struct EventsParams {
    day: String,
}

/// GET /api/v1/playback/events -- attendance events for timeline markers.
async fn events_handler(
    State(state): State<Arc<PlaybackState>>,
    Query(params): Query<EventsParams>,
) -> impl IntoResponse {
    let db_path = state.db_path.clone();
    let day = params.day;

    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let conn =
            rusqlite::Connection::open(&db_path).map_err(|e| format!("db open: {e}"))?;
        let entries = attendance::db::get_attendance_for_day(&conn, &day)
            .map_err(|e| format!("get_attendance_for_day: {e}"))?;
        let count = entries.len();
        Ok(json!({
            "day": day,
            "events": entries,
            "count": count,
        }))
    })
    .await;

    match result {
        Ok(Ok(val)) => Json(val).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
