use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Json;
use bytes::Bytes;
use futures::stream::unfold;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{Mutex, RwLock};
use tower_http::cors::{Any, CorsLayer};

use crate::config::CameraConfig;
use crate::detection::decoder::FrameDecoder;
use crate::frame::FrameBuffer;
use crate::nvr::NvrClient;

/// Cached snapshot with timestamp for staleness detection.
struct CachedSnapshot {
    data: Bytes,
    #[allow(dead_code)]
    fetched_at: Instant,
}

/// Background-refreshed snapshot cache for all NVR channels.
pub struct SnapshotCache {
    entries: RwLock<HashMap<u32, CachedSnapshot>>,
}

impl SnapshotCache {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Get a cached snapshot. Returns None if no snapshot has been fetched yet.
    pub async fn get(&self, channel: u32) -> Option<Bytes> {
        let guard = self.entries.read().await;
        guard.get(&channel).map(|s| s.data.clone())
    }

    /// Update the cache for a channel.
    async fn set(&self, channel: u32, data: Bytes) {
        let mut guard = self.entries.write().await;
        guard.insert(channel, CachedSnapshot {
            data,
            fetched_at: Instant::now(),
        });
    }
}

/// Spawn a background task that continuously fetches snapshots from the NVR
/// and populates the cache. Fetches in parallel batches of 3 to reduce cycle
/// time from ~16s (sequential) to ~5s while respecting NVR connection limits.
pub fn spawn_snapshot_fetcher(
    nvr: Arc<NvrClient>,
    cache: Arc<SnapshotCache>,
    channels: u32,
) {
    const BATCH_SIZE: u32 = 3;

    tokio::spawn(async move {
        tracing::info!(channels, "NVR snapshot fetcher started (batch size {})", BATCH_SIZE);
        loop {
            let mut ch = 1;
            while ch <= channels {
                let batch_end = (ch + BATCH_SIZE).min(channels + 1);
                let mut handles = Vec::new();

                for c in ch..batch_end {
                    let nvr_clone = Arc::clone(&nvr);
                    handles.push(tokio::spawn(async move {
                        (c, nvr_clone.snapshot(c).await)
                    }));
                }

                for handle in handles {
                    if let Ok((c, result)) = handle.await {
                        match result {
                            Ok(bytes) => { cache.set(c, bytes).await; }
                            Err(e) => {
                                tracing::debug!(channel = c, error = %e, "snapshot fetch failed");
                            }
                        }
                    }
                }

                ch = batch_end;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });
}

/// Layout preferences stored in camera-layout.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraLayout {
    #[serde(default = "default_grid_mode")]
    pub grid_mode: String,
    #[serde(default)]
    pub camera_order: Vec<u32>,
    #[serde(default)]
    pub zone_filter: Option<String>,
}

fn default_grid_mode() -> String {
    "3x3".to_string()
}

impl Default for CameraLayout {
    fn default() -> Self {
        Self {
            grid_mode: default_grid_mode(),
            camera_order: Vec::new(),
            zone_filter: None,
        }
    }
}

/// Shared mutable layout state backed by camera-layout.json.
pub struct LayoutState {
    pub layout: Mutex<CameraLayout>,
    pub file_path: PathBuf,
}

impl LayoutState {
    /// Load layout from file, or return defaults if file doesn't exist or is invalid.
    pub fn load(file_path: PathBuf) -> Self {
        let layout = match std::fs::read_to_string(&file_path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => CameraLayout::default(),
        };
        Self {
            layout: Mutex::new(layout),
            file_path,
        }
    }
}

/// Shared state for MJPEG streaming endpoints.
pub struct MjpegState {
    pub frame_buf: FrameBuffer,
    pub cameras: Vec<CameraConfig>,
    #[allow(dead_code)]
    pub service_port: u16,
    pub nvr_channels: u32,
    pub snapshot_cache: Arc<SnapshotCache>,
    pub layout_state: Arc<LayoutState>,
}

#[derive(Serialize)]
struct CameraInfo {
    name: String,
    display_name: String,
    display_order: u32,
    role: String,
    zone: String,
    nvr_channel: Option<u32>,
    stream_url: String,
    status: &'static str,
}

/// Build the MJPEG router with CORS enabled for cross-origin dashboard access.
pub fn mjpeg_router(state: Arc<MjpegState>) -> axum::Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::PUT])
        .allow_headers(Any);

    axum::Router::new()
        .route("/cameras/live", get(cameras_page_handler))
        .route("/api/v1/cameras", get(cameras_list_handler))
        .route("/api/v1/cameras/:name/stream", get(mjpeg_stream_handler))
        .route(
            "/api/v1/cameras/nvr/:channel/snapshot",
            get(nvr_snapshot_handler),
        )
        .route(
            "/api/v1/cameras/layout",
            get(layout_get_handler).put(layout_put_handler),
        )
        .with_state(state)
        .layer(cors)
}

/// GET /cameras -- serves the all-cameras dashboard page.
async fn cameras_page_handler() -> Response {
    const HTML: &str = include_str!("../cameras.html");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(HTML))
        .expect("valid response")
}

/// GET /api/v1/cameras/nvr/:channel/snapshot -- serves a cached JPEG snapshot.
///
/// Snapshots are pre-fetched by a background task, so this handler returns
/// instantly from cache. Returns 404 for invalid channels, 503 if no snapshot
/// has been cached yet for the requested channel.
async fn nvr_snapshot_handler(
    State(state): State<Arc<MjpegState>>,
    Path(channel): Path<u32>,
) -> Response {
    if channel < 1 || channel > state.nvr_channels {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "invalid channel number"})),
        )
            .into_response();
    }

    match state.snapshot_cache.get(channel).await {
        Some(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/jpeg")
            .header(header::CACHE_CONTROL, "no-cache, no-store")
            .body(Body::from(bytes))
            .expect("valid response"),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "snapshot not yet available"})),
        )
            .into_response(),
    }
}

/// GET /api/v1/cameras -- returns JSON list of configured cameras with stream URLs and status.
async fn cameras_list_handler(State(state): State<Arc<MjpegState>>) -> Json<serde_json::Value> {
    let now = Instant::now();
    let mut cameras = Vec::with_capacity(state.cameras.len());

    for cam in &state.cameras {
        let status = match state.frame_buf.get(&cam.name).await {
            Some(frame) => {
                let age = now.duration_since(frame.timestamp).as_secs_f64();
                if age < 10.0 {
                    "connected"
                } else if age < 30.0 {
                    "reconnecting"
                } else {
                    "disconnected"
                }
            }
            None => "offline",
        };

        cameras.push(CameraInfo {
            name: cam.name.clone(),
            display_name: cam.effective_display_name().to_string(),
            display_order: cam.display_order.unwrap_or(0),
            role: cam.role.clone(),
            zone: cam.zone.clone(),
            nvr_channel: cam.nvr_channel,
            stream_url: format!("/api/v1/cameras/{}/stream", cam.name),
            status,
        });
    }

    Json(json!(cameras))
}

/// GET /api/v1/cameras/:name/stream -- serves MJPEG multipart stream.
///
/// Uses a per-connection H.264 decoder to convert NAL units from FrameBuffer
/// into JPEG frames. The decoder is stateful: P-frames that arrive before the
/// first I-frame are silently skipped until a keyframe is received.
///
/// CRITICAL: Only calls frame_buf.get() (read lock) -- zero write contention
/// with the AI detection pipeline.
async fn mjpeg_stream_handler(
    State(state): State<Arc<MjpegState>>,
    Path(name): Path<String>,
) -> Response {
    // Verify camera exists in config
    let camera = match state.cameras.iter().find(|c| c.name == name) {
        Some(cam) => cam.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "camera not found"})),
            )
                .into_response();
        }
    };

    // Cap FPS to 10 to avoid overwhelming browser clients
    let fps = camera.fps.min(10).max(1);
    let frame_interval = Duration::from_millis(1000 / fps as u64);

    // Stream state: H.264 decoder + last frame counter to avoid re-encoding same frame
    struct StreamState {
        state: Arc<MjpegState>,
        camera_name: String,
        decoder: Option<FrameDecoder>,
        last_frame_count: u64,
        frame_interval: Duration,
    }

    let initial = StreamState {
        state: state.clone(),
        camera_name: name,
        decoder: FrameDecoder::new().ok(),
        last_frame_count: 0,
        frame_interval,
    };

    let stream = unfold(initial, |mut s| async move {
        // Sleep for frame interval to maintain target FPS
        tokio::time::sleep(s.frame_interval).await;

        let frame_data = s.state.frame_buf.get(&s.camera_name).await;

        let jpeg_bytes = match frame_data {
            Some(fd) => {
                // Skip if same frame we already sent
                if fd.frame_count == s.last_frame_count && s.last_frame_count > 0 {
                    return Some((Ok::<Bytes, std::io::Error>(Bytes::new()), s));
                }
                s.last_frame_count = fd.frame_count;

                // Decode H.264 NAL to RGB, then encode to JPEG
                let decoder = match s.decoder.as_mut() {
                    Some(d) => d,
                    None => {
                        // Decoder failed to init, send empty
                        return Some((Ok(Bytes::new()), s));
                    }
                };

                match decoder.decode(&fd.data) {
                    Ok(Some(decoded)) => {
                        // Encode RGB to JPEG using image crate
                        match encode_rgb_to_jpeg(&decoded.rgb, decoded.width, decoded.height) {
                            Some(jpeg) => jpeg,
                            None => return Some((Ok(Bytes::new()), s)),
                        }
                    }
                    Ok(None) => {
                        // Waiting for keyframe, skip
                        return Some((Ok(Bytes::new()), s));
                    }
                    Err(_) => {
                        // Decode error, skip frame
                        return Some((Ok(Bytes::new()), s));
                    }
                }
            }
            None => {
                // No frame available yet
                return Some((Ok(Bytes::new()), s));
            }
        };

        // Format as MJPEG multipart boundary
        let boundary = format!(
            "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
            jpeg_bytes.len()
        );

        let mut part = Vec::with_capacity(boundary.len() + jpeg_bytes.len() + 2);
        part.extend_from_slice(boundary.as_bytes());
        part.extend_from_slice(&jpeg_bytes);
        part.extend_from_slice(b"\r\n");

        Some((Ok(Bytes::from(part)), s))
    });

    // Filter out empty frames (no-ops when waiting for keyframe or same frame)
    let stream = futures::StreamExt::filter_map(stream, |result| async move {
        match result {
            Ok(bytes) if bytes.is_empty() => None,
            other => Some(other),
        }
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )
        .header(header::CACHE_CONTROL, "no-cache, no-store")
        .header(header::CONNECTION, "keep-alive")
        .body(Body::from_stream(stream))
        .expect("valid response")
}

/// GET /api/v1/cameras/layout — returns current layout preferences.
async fn layout_get_handler(State(state): State<Arc<MjpegState>>) -> Json<serde_json::Value> {
    let layout = state.layout_state.layout.lock().await;
    Json(json!({
        "grid_mode": layout.grid_mode,
        "camera_order": layout.camera_order,
        "zone_filter": layout.zone_filter,
    }))
}

/// PUT /api/v1/cameras/layout — saves layout preferences atomically to camera-layout.json.
async fn layout_put_handler(
    State(state): State<Arc<MjpegState>>,
    Json(incoming): Json<CameraLayout>,
) -> Response {
    // Update in-memory state
    {
        let mut layout = state.layout_state.layout.lock().await;
        *layout = incoming.clone();
    }

    // Atomic write: write to temp file, then rename
    let tmp_path = state.layout_state.file_path.with_extension("json.tmp");
    let json_bytes = match serde_json::to_string_pretty(&incoming) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "failed to serialize layout");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "serialization failed"})),
            )
                .into_response();
        }
    };

    if let Err(e) = tokio::fs::write(&tmp_path, json_bytes.as_bytes()).await {
        tracing::error!(error = %e, path = %tmp_path.display(), "failed to write temp layout file");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "failed to write layout file"})),
        )
            .into_response();
    }

    if let Err(e) = tokio::fs::rename(&tmp_path, &state.layout_state.file_path).await {
        tracing::error!(error = %e, "failed to rename temp layout file");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "failed to persist layout file"})),
        )
            .into_response();
    }

    tracing::info!(path = %state.layout_state.file_path.display(), "camera layout saved");
    Json(json!({"status": "ok"})).into_response()
}

/// Encode raw RGB bytes to JPEG.
fn encode_rgb_to_jpeg(rgb: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    use image::codecs::jpeg::JpegEncoder;
    use std::io::Cursor;

    let mut buf = Cursor::new(Vec::with_capacity(width as usize * height as usize));
    let mut encoder = JpegEncoder::new_with_quality(&mut buf, 70);
    encoder
        .encode(rgb, width, height, image::ExtendedColorType::Rgb8)
        .ok()?;
    Some(buf.into_inner())
}
