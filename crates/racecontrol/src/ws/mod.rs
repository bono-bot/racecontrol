mod agent_handler;
mod dashboard_handler;
mod ai_handler;
mod spectator_handler;

pub use dashboard_handler::handle_dashboard;
pub use ai_handler::{ai_ws, ws_exec_on_pod};
pub use spectator_handler::spectator_ws;

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::time::Duration;

// Phase 206 (OBS-04): Rate-limit sentinel WhatsApp alerts to 1 per sentinel type per pod per 5 min.
// Key format: "sentinel_{file}_{pod_number}". Prevents alert storms during restart storms.
static SENTINEL_ALERT_COOLDOWN: std::sync::LazyLock<Mutex<HashMap<String, Instant>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

// Reconnect storm throttle: prevent the same pod from re-registering within 2 seconds.
// Normal reconnects take 5-10s minimum. Rapid re-registration indicates a reconnect storm
// (e.g., all 8 pods restarted simultaneously) that can crash the server.
static REGISTER_COOLDOWN: std::sync::LazyLock<Mutex<HashMap<String, Instant>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

const REGISTER_COOLDOWN_SECS: u64 = 2;

/// Phase 364 GLD-D-05: Counter for try_send overflow events on hot-path WS channels.
/// Incremented when try_send returns TrySendError::Full (channel full, message dropped).
/// Read by metrics_producers.rs every 30s and flushed to metrics_samples table.
pub static WS_TRY_SEND_OVERFLOWS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// MI Phase 1: Count of currently connected dashboard WebSocket clients.
/// Incremented on WS connect, decremented on disconnect.
/// Used by detect_fleet_anomalies() to detect "kiosk healthy but no WS clients" (DASHBOARD_ORPHAN).
static DASHBOARD_CLIENT_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Rolling count of WS connect events in the last 60s. High churn = stale frontend build.
static DASHBOARD_WS_CONNECTS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// Rolling count of WS disconnect events in the last 60s.
static DASHBOARD_WS_DISCONNECTS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Get the current number of connected dashboard WS clients.
pub fn dashboard_client_count() -> u32 {
    DASHBOARD_CLIENT_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Get WS churn metrics (connects, disconnects in rolling window).
/// Churn > 10/min indicates a client in a connect/disconnect loop (stale frontend build).
pub fn dashboard_ws_churn() -> (u32, u32) {
    (
        DASHBOARD_WS_CONNECTS.load(std::sync::atomic::Ordering::Relaxed),
        DASHBOARD_WS_DISCONNECTS.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// Reset churn counters (called every 60s by a background task).
pub fn reset_ws_churn_counters() {
    DASHBOARD_WS_CONNECTS.store(0, std::sync::atomic::Ordering::Relaxed);
    DASHBOARD_WS_DISCONNECTS.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// WS-HARDEN: Track failed WS auth attempts per source. 5 failures in 5min = lockout.
static WS_AUTH_FAILURES: std::sync::LazyLock<Mutex<HashMap<String, (u32, Instant)>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Check if a WS source is locked out from auth failures. Returns true if locked out.
fn ws_auth_locked_out(source: &str) -> bool {
    let map = WS_AUTH_FAILURES.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((count, first_failure)) = map.get(source) {
        if first_failure.elapsed() < Duration::from_secs(300) && *count >= 5 {
            return true;
        }
    }
    false
}

/// Record a failed WS auth attempt.
fn ws_auth_record_failure(source: &str) {
    let mut map = WS_AUTH_FAILURES.lock().unwrap_or_else(|p| p.into_inner());
    let entry = map.entry(source.to_string()).or_insert((0, Instant::now()));
    if entry.1.elapsed() > Duration::from_secs(300) {
        // Reset window
        *entry = (1, Instant::now());
    } else {
        entry.0 += 1;
    }
}

/// Check and update sentinel alert cooldown. Returns true if the alert should fire.
/// Cooldown key: `sentinel_{file}_{pod_number}`. Cooldown period: 300s (5 minutes).
fn check_sentinel_cooldown(key: &str) -> bool {
    const COOLDOWN_SECS: u64 = 300;
    let mut map = SENTINEL_ALERT_COOLDOWN.lock().unwrap_or_else(|p| p.into_inner());
    let now = Instant::now();
    if let Some(last) = map.get(key) {
        if now.duration_since(*last).as_secs() < COOLDOWN_SECS {
            return false;
        }
    }
    map.insert(key.to_string(), now);
    true
}

use crate::state::AppState;

/// Known MAC addresses for WOL — keyed by pod ID.
fn pod_mac_address(pod_id: &str) -> Option<String> {
    match pod_id {
        "pod_1" => Some("30:56:0F:05:45:88".into()),
        "pod_2" => Some("30:56:0F:05:46:53".into()),
        "pod_3" => Some("30:56:0F:05:44:B3".into()),
        "pod_4" => Some("30:56:0F:05:45:25".into()),
        "pod_5" => Some("30:56:0F:05:44:B7".into()),
        "pod_6" => Some("30:56:0F:05:45:6E".into()),
        "pod_7" => Some("30:56:0F:05:44:B4".into()),
        "pod_8" => Some("30:56:0F:05:46:C5".into()),
        _ => None,
    }
}

/// Query parameters for WS authentication
#[derive(serde::Deserialize, Default)]
pub struct WsAuthParams {
    /// PSK bootstrap token — must match config.cloud.terminal_secret
    #[serde(default)]
    pub(crate) token: Option<String>,
    /// Per-pod JWT token — issued by server after first PSK auth (Phase 306)
    #[serde(default)]
    pub(crate) jwt: Option<String>,
}

/// Validate WebSocket token against terminal_secret (if configured).
/// Returns true if: no secret configured (dev mode), or token matches.
pub(super) fn verify_ws_token(state: &AppState, token: &Option<String>) -> bool {
    match &state.config.cloud.terminal_secret {
        None => true, // dev mode — no auth required
        Some(secret) if secret.is_empty() => true,
        Some(secret) => token.as_deref() == Some(secret.as_str()),
    }
}

/// WebSocket endpoint for pod agents
pub async fn agent_ws(
    Query(params): Query<WsAuthParams>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    match agent_handler::authenticate_agent_ws(&state, &params) {
        Ok(auth_result) => Ok(ws.on_upgrade(move |socket| agent_handler::handle_agent(socket, state, auth_result))),
        Err(reason) => {
            tracing::warn!("WS agent connection rejected — {}", reason);
            // WSAUTH-03: WhatsApp alert on invalid JWT (not PSK — too noisy)
            if params.jwt.as_ref().map_or(false, |j| !j.is_empty()) {
                let state_clone = state.clone();
                let reason_clone = reason.clone();
                tokio::spawn(async move {
                    crate::whatsapp_alerter::send_admin_alert(
                        &state_clone.config,
                        "ws_jwt_rejected",
                        &format!("Pod WS connection rejected: {}", reason_clone),
                    ).await;
                });
            }
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// WebSocket endpoint for dashboard clients.
/// Accepts token via query param (`?token=...`) OR Sec-WebSocket-Protocol header.
/// The kiosk sends the token via protocol header (NEXT_PUBLIC_WS_TOKEN),
/// while direct browser connections use the query param.
pub async fn dashboard_ws(
    Query(params): Query<WsAuthParams>,
    headers: axum::http::HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    // Try query param first, then Sec-WebSocket-Protocol header
    let mut token = params.token.clone();
    if token.is_none() || token.as_deref() == Some("") {
        token = headers
            .get("sec-websocket-protocol")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
    }
    // WS-HARDEN: rate limit failed auth attempts (5 failures in 5min = lockout)
    let source = token.as_deref().unwrap_or("no-token").chars().take(8).collect::<String>();
    if ws_auth_locked_out(&source) {
        tracing::warn!("WS dashboard connection rate-limited — source '{}' locked out", source);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if !verify_ws_token(&state, &token) {
        ws_auth_record_failure(&source);
        tracing::warn!("WS dashboard connection rejected — invalid or missing token");
        return Err(StatusCode::UNAUTHORIZED);
    }
    // Echo back the Sec-WebSocket-Protocol so browsers don't close the connection.
    let mut response = ws.on_upgrade(|socket| handle_dashboard(socket, state)).into_response();
    if let Some(proto) = headers.get("sec-websocket-protocol") {
        response.headers_mut().insert("sec-websocket-protocol", proto.clone());
    }
    Ok(response)
}
