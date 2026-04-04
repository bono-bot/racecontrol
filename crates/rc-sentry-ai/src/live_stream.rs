//! On-demand NVR live streaming — eliminates go2rtc dependency.
//!
//! Two endpoints:
//! - `/api/v1/stream/mjpeg/:channel` — MJPEG proxy (all browsers, D1 quality)
//! - `/api/v1/stream/ws/:channel`    — H.265 WebSocket (Chrome/Edge, 4MP native)
//!
//! Both are on-demand: NVR connections open when a viewer connects,
//! close when the last viewer disconnects.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::body::Body;
use futures::StreamExt;
use serde::Deserialize;
use tower_http::cors::{Any, CorsLayer};

use crate::config::NvrConfig;
use crate::nvr;

/// Shared state for live stream endpoints.
pub struct LiveStreamState {
    pub nvr_host: String,
    pub nvr_port: u16,
    pub nvr_username: String,
    pub nvr_password: String,
}

impl LiveStreamState {
    pub fn from_nvr_config(config: &NvrConfig) -> Self {
        Self {
            nvr_host: config.host.clone(),
            nvr_port: config.port,
            nvr_username: config.username.clone(),
            nvr_password: config.password.clone(),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}:{}", self.nvr_host, self.nvr_port)
    }

    fn rtsp_url(&self, channel: u32, subtype: u32) -> String {
        // Properly URL-encode username and password (handles @, /, ?, &, etc.)
        let encoded_user = url_encode_component(&self.nvr_username);
        let encoded_pass = url_encode_component(&self.nvr_password);
        format!(
            "rtsp://{}:{}@{}:554/cam/realmonitor?channel={}&subtype={}",
            encoded_user, encoded_pass, self.nvr_host, channel, subtype
        )
    }

    /// Redacted RTSP URL safe for logging (no credentials).
    fn rtsp_url_redacted(&self, channel: u32, subtype: u32) -> String {
        format!(
            "rtsp://***:***@{}:554/cam/realmonitor?channel={}&subtype={}",
            self.nvr_host, channel, subtype
        )
    }
}

/// URL-encode a string component (RFC 3986 unreserved chars pass through).
fn url_encode_component(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

#[derive(Deserialize)]
pub struct StreamParams {
    /// RTSP subtype: 0 = main stream (4MP H.265), 1 = sub stream (D1)
    #[serde(default = "default_subtype")]
    pub subtype: u32,
}

fn default_subtype() -> u32 {
    1
}

/// Build the live stream router.
pub fn live_stream_router(state: Arc<LiveStreamState>) -> axum::Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET])
        .allow_headers(Any);

    axum::Router::new()
        .route("/api/v1/stream/mjpeg/:channel", get(mjpeg_proxy))
        .route("/api/v1/stream/ws/:channel", get(ws_live_handler))
        .with_state(state)
        .layer(cors)
}

// ── MJPEG Proxy ──────────────────────────────────────────────────────────────

/// Proxy MJPEG stream from NVR to browser with transparent digest auth.
/// On-demand: connection to NVR opens on request, closes when browser disconnects.
async fn mjpeg_proxy(
    State(state): State<Arc<LiveStreamState>>,
    Path(channel): Path<u32>,
    Query(params): Query<StreamParams>,
) -> Response {
    let url = format!(
        "{}/cgi-bin/mjpg/video.cgi?channel={}&subtype={}",
        state.base_url(),
        channel,
        params.subtype
    );

    // Build a client with no timeout for long-lived streaming
    let client = match reqwest::Client::builder().build() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "failed to build HTTP client for MJPEG proxy");
            return (StatusCode::INTERNAL_SERVER_ERROR, "client build error").into_response();
        }
    };

    // Perform digest auth handshake
    let resp = match digest_get_stream(&client, &url, &state.nvr_username, &state.nvr_password).await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, channel, "MJPEG proxy: NVR connection failed");
            return (StatusCode::BAD_GATEWAY, format!("NVR error: {e}")).into_response();
        }
    };

    // Forward content-type from NVR (multipart/x-mixed-replace)
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("multipart/x-mixed-replace; boundary=myboundary")
        .to_string();

    tracing::info!(channel, subtype = params.subtype, "MJPEG proxy stream opened");

    // Stream the NVR response body directly to browser
    let body = Body::from_stream(resp.bytes_stream());

    // CORS handled by CorsLayer middleware — no manual header needed
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-cache, no-store")
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

// ── WebSocket H.265 Live Stream ──────────────────────────────────────────────

/// WebSocket upgrade handler for H.265 live streaming.
/// Opens an RTSP connection to NVR on-demand, streams raw H.265 frames.
async fn ws_live_handler(
    State(state): State<Arc<LiveStreamState>>,
    Path(channel): Path<u32>,
    Query(params): Query<StreamParams>,
    ws: WebSocketUpgrade,
) -> Response {
    let rtsp_url = state.rtsp_url(channel, params.subtype);
    let rtsp_url_safe = state.rtsp_url_redacted(channel, params.subtype);
    let subtype = params.subtype;
    tracing::info!(channel, subtype, "WebSocket live stream requested");
    ws.on_upgrade(move |socket| ws_live_stream(socket, rtsp_url, rtsp_url_safe, channel, subtype))
}

/// WebSocket live stream task: pulls RTSP from NVR, sends H.265 frames.
///
/// Protocol:
/// 1. First message (text/JSON): `{"type":"init","codec":"hev1.1.6.L123.B0","width":2560,"height":1440}`
/// 2. Subsequent messages (binary): `[u64 LE timestamp_µs][u8 flags][H.265 Annex B NALUs]`
///    flags: 0x01 = keyframe
async fn ws_live_stream(mut socket: WebSocket, rtsp_url: String, rtsp_url_safe: String, channel: u32, subtype: u32) {
    // Resolution depends on NVR subtype: 0 = main (4MP), 1 = sub (D1/CIF)
    let (width, height, codec) = match subtype {
        0 => (2560, 1440, "hev1.1.6.L123.B0"),  // 4MP H.265 main stream
        _ => (704, 576, "hev1.1.6.L93.B0"),       // D1 H.265 sub stream
    };

    // Send init message with codec info
    let init_msg = serde_json::json!({
        "type": "init",
        "codec": codec,
        "width": width,
        "height": height
    });
    if socket
        .send(Message::Text(init_msg.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    // Connect to NVR RTSP (use redacted URL in all logs — never log credentials)
    let url = match url::Url::parse(&rtsp_url) {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(error = %e, channel, url = %rtsp_url_safe, "invalid RTSP URL");
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({"type":"error","msg":"invalid RTSP URL"})
                        .to_string()
                        .into(),
                ))
                .await;
            return;
        }
    };

    let session_group = Arc::new(retina::client::SessionGroup::default());
    let mut session = match retina::client::Session::describe(
        url,
        retina::client::SessionOptions::default().session_group(session_group),
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, channel, url = %rtsp_url_safe, "RTSP DESCRIBE failed");
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({"type":"error","msg":"RTSP connection failed"})
                        .to_string()
                        .into(),
                ))
                .await;
            return;
        }
    };

    // Setup video track with Annex B framing
    if let Err(e) = session
        .setup(
            0,
            retina::client::SetupOptions::default()
                .transport(retina::client::Transport::Tcp(
                    retina::client::TcpTransportOptions::default(),
                ))
                .frame_format(retina::codec::FrameFormat::SIMPLE),
        )
        .await
    {
        tracing::error!(error = %e, channel, "RTSP SETUP failed");
        return;
    }

    let mut session = match session
        .play(retina::client::PlayOptions::default())
        .await
    {
        Ok(s) => match s.demuxed() {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(error = %e, channel, "RTSP demux failed");
                return;
            }
        },
        Err(e) => {
            tracing::error!(error = %e, channel, "RTSP PLAY failed");
            return;
        }
    };

    tracing::info!(channel, "WebSocket H.265 stream connected to NVR");

    let mut frame_count: u64 = 0;
    let mut dropped_frames: u64 = 0;
    let start = std::time::Instant::now();
    let mut last_frame_time = std::time::Instant::now();
    /// Minimum interval between frames sent to WS (25fps = 40ms).
    /// Frames arriving faster are dropped to avoid overwhelming slow clients.
    const MIN_FRAME_INTERVAL: Duration = Duration::from_millis(40);
    /// Maximum pending WS send time before considering client too slow.
    /// 2s (down from 5s) for faster recovery — stalled clients reconnect sooner
    /// instead of accumulating a multi-second frame backlog.
    const WS_SEND_TIMEOUT: Duration = Duration::from_secs(2);

    // Stream frames to WebSocket
    loop {
        tokio::select! {
            // Read from RTSP
            item = session.next() => {
                match item {
                    Some(Ok(retina::codec::CodecItem::VideoFrame(frame))) => {
                        let is_key = frame.is_random_access_point();

                        // Frame pacing: skip non-keyframes that arrive faster than 25fps.
                        // Always send keyframes to avoid decoder stalls.
                        if !is_key && last_frame_time.elapsed() < MIN_FRAME_INTERVAL {
                            dropped_frames += 1;
                            continue;
                        }

                        let timestamp_us = start.elapsed().as_micros() as u64;
                        let data = frame.data();

                        // Build binary message: [8-byte timestamp][1-byte flags][H.265 data]
                        let mut msg = Vec::with_capacity(9 + data.len());
                        msg.extend_from_slice(&timestamp_us.to_le_bytes());
                        msg.push(if is_key { 0x01 } else { 0x00 });
                        msg.extend_from_slice(data);

                        // Backpressure: timeout WS send to detect slow clients.
                        // If WS can't keep up, disconnect rather than buffer unbounded.
                        let send_result = tokio::time::timeout(
                            WS_SEND_TIMEOUT,
                            socket.send(Message::Binary(msg.into())),
                        ).await;

                        match send_result {
                            Ok(Ok(())) => {
                                frame_count += 1;
                                last_frame_time = std::time::Instant::now();
                            }
                            Ok(Err(_)) => {
                                tracing::info!(channel, frames = frame_count, "WebSocket client disconnected");
                                break;
                            }
                            Err(_) => {
                                tracing::warn!(channel, frames = frame_count, "WebSocket send timeout — slow client, disconnecting");
                                break;
                            }
                        }
                    }
                    Some(Ok(_)) => {
                        // Skip audio/metadata
                    }
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, channel, "RTSP stream error");
                        let _ = socket.send(Message::Text(
                            serde_json::json!({"type":"error","msg":"stream error"})
                                .to_string().into()
                        )).await;
                        break;
                    }
                    None => {
                        tracing::info!(channel, "RTSP stream ended");
                        break;
                    }
                }
            }
            // Check for WebSocket close/ping
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::info!(channel, frames = frame_count, "WebSocket closed by client");
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    _ => {}
                }
            }
        }
    }

    if dropped_frames > 0 {
        tracing::debug!(channel, dropped_frames, "frames dropped due to rate limiting");
    }

    tracing::info!(channel, frames = frame_count, "live stream session ended, RTSP connection closed");
    // RTSP connection drops automatically when `session` is dropped (on-demand!)
}

// ── Digest Auth Helper ───────────────────────────────────────────────────────

/// Perform a GET request with digest auth, returning the streaming response.
async fn digest_get_stream(
    client: &reqwest::Client,
    url: &str,
    username: &str,
    password: &str,
) -> anyhow::Result<reqwest::Response> {
    let uri = url::Url::parse(url)?;
    let uri_path = if let Some(q) = uri.query() {
        format!("{}?{}", uri.path(), q)
    } else {
        uri.path().to_string()
    };

    // First try unauthenticated to get the digest challenge
    let resp = client.get(url).send().await?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        let www_auth = resp
            .headers()
            .get("www-authenticate")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow::anyhow!("no WWW-Authenticate header"))?;

        let challenge = nvr::parse_digest_challenge(www_auth)?;
        let nc = "00000001";
        let cnonce = format!("{:08x}", nvr::rand_cnonce());
        let auth_header = nvr::compute_digest_header(
            username,
            password,
            &challenge.realm,
            &challenge.nonce,
            &challenge.qop,
            "GET",
            &uri_path,
            nc,
            &cnonce,
        );

        let resp = client
            .get(url)
            .header("Authorization", auth_header)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("NVR digest auth failed: {}", resp.status());
        }
        Ok(resp)
    } else if resp.status().is_success() {
        Ok(resp)
    } else {
        anyhow::bail!("NVR request failed: {}", resp.status());
    }
}
