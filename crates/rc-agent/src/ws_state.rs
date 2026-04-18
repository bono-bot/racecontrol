//! WebSocket connection state snapshot for remote diagnostics.
//!
//! Exposes `GET /debug/ws-state` on the rc-agent HTTP server so the WS-client
//! reconnect loop can be diagnosed remotely without needing to read the rc-agent
//! JSONL (which can exceed the `/file` 50 MB limit on a busy day).
//!
//! Motivated by the 2026-04-18 incident: Pod 1 + Pod 6 WS-reconnect-forever.
//! rc-agent HTTP stayed alive for 53 minutes while the reconnect loop silently
//! failed every attempt; the only external signal was `ws_reconnect_count=0` in
//! server-side fleet/health. This endpoint surfaces the last connect error and
//! the consecutive-failure counter so the same class of failure becomes
//! diagnosable in <30 seconds next time.
//!
//! Design:
//!   - Single shared state (Arc<RwLock<Inner>>) behind a OnceLock, mirroring the
//!     existing DIAG_LOG pattern in remote_ops.rs. No router-state plumbing —
//!     the handler reads the static directly.
//!   - Non-blocking updates via `blocking_write` are NOT used; the WS reconnect
//!     loop uses `write().await` which yields.
//!   - Monotonic counters (attempt_count, consecutive_failures) persist across
//!     the reconnect-loop lifetime; they reset only on process restart.

use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

static WS_STATE: std::sync::OnceLock<Arc<RwLock<WsStateInner>>> = std::sync::OnceLock::new();

/// Phase of the WS client as last observed by the reconnect loop.
/// String-typed (not enum) so callers can grep the `/debug/ws-state` response
/// without needing to track a new type across rc-agent and tooling.
#[derive(Default)]
pub struct WsStateInner {
    /// One of: "init" | "connecting" | "connected" | "disconnected".
    /// "connected" means `connect_async` resolved Ok; it does NOT guarantee
    /// the server processed the Register message — that's a separate signal
    /// (server-side `ws_reconnect_count` in fleet/health).
    pub current_phase: String,
    /// Lifetime count of connect attempts since process start.
    pub attempt_count: u32,
    /// Lifetime count of successful `connect_async` resolutions.
    pub success_count: u32,
    /// Consecutive failures since last success. Resets to 0 on connect Ok.
    pub consecutive_failures: u32,
    /// Most recent error string from a failed connect attempt (truncated at 500 chars).
    pub last_connect_error: Option<String>,
    /// ISO-8601 UTC timestamp of the last connect error.
    pub last_connect_error_at: Option<String>,
    /// ISO-8601 UTC timestamp of the last successful connect.
    pub last_successful_connect_at: Option<String>,
    /// ISO-8601 UTC timestamp of the last connect attempt (success or failure).
    pub last_attempt_at: Option<String>,
    /// URL last attempted (auth query-string redacted).
    pub last_connect_url: Option<String>,
}

/// JSON shape returned by `GET /debug/ws-state`.
#[derive(Serialize)]
pub struct WsStateSnapshot {
    pub current_phase: String,
    pub attempt_count: u32,
    pub success_count: u32,
    pub consecutive_failures: u32,
    pub last_connect_error: Option<String>,
    pub last_connect_error_at: Option<String>,
    pub last_successful_connect_at: Option<String>,
    pub last_attempt_at: Option<String>,
    pub last_connect_url: Option<String>,
}

/// Initialize the shared WS state. Call once from main() before the reconnect
/// loop starts. Idempotent — subsequent calls are no-ops.
pub fn init() {
    let _ = WS_STATE.set(Arc::new(RwLock::new(WsStateInner {
        current_phase: "init".to_string(),
        ..Default::default()
    })));
}

/// Record the start of a connect attempt. Increments `attempt_count`, sets
/// `current_phase=connecting`, stamps `last_attempt_at`, stores redacted URL.
pub async fn record_attempt(url_with_auth: &str) {
    if let Some(state) = WS_STATE.get() {
        let mut w = state.write().await;
        w.attempt_count = w.attempt_count.saturating_add(1);
        w.current_phase = "connecting".to_string();
        w.last_attempt_at = Some(chrono::Utc::now().to_rfc3339());
        w.last_connect_url = Some(redact_url(url_with_auth));
    }
}

/// Record a successful `connect_async`. Resets consecutive_failures.
pub async fn record_success() {
    if let Some(state) = WS_STATE.get() {
        let mut w = state.write().await;
        w.success_count = w.success_count.saturating_add(1);
        w.consecutive_failures = 0;
        w.current_phase = "connected".to_string();
        w.last_successful_connect_at = Some(chrono::Utc::now().to_rfc3339());
    }
}

/// Record a failed connect (error or timeout). Increments consecutive_failures.
pub async fn record_failure(err: &str) {
    if let Some(state) = WS_STATE.get() {
        let mut w = state.write().await;
        w.consecutive_failures = w.consecutive_failures.saturating_add(1);
        w.current_phase = "disconnected".to_string();
        let truncated: String = err.chars().take(500).collect();
        w.last_connect_error = Some(truncated);
        w.last_connect_error_at = Some(chrono::Utc::now().to_rfc3339());
    }
}

/// Mark the connection as dropped (inner message loop exited). Leaves
/// success/failure counters untouched; caller records the next connect
/// attempt separately.
pub async fn record_disconnected() {
    if let Some(state) = WS_STATE.get() {
        let mut w = state.write().await;
        w.current_phase = "disconnected".to_string();
    }
}

/// Snapshot the current state for JSON serialization.
pub async fn snapshot() -> WsStateSnapshot {
    if let Some(state) = WS_STATE.get() {
        let r = state.read().await;
        WsStateSnapshot {
            current_phase: r.current_phase.clone(),
            attempt_count: r.attempt_count,
            success_count: r.success_count,
            consecutive_failures: r.consecutive_failures,
            last_connect_error: r.last_connect_error.clone(),
            last_connect_error_at: r.last_connect_error_at.clone(),
            last_successful_connect_at: r.last_successful_connect_at.clone(),
            last_attempt_at: r.last_attempt_at.clone(),
            last_connect_url: r.last_connect_url.clone(),
        }
    } else {
        WsStateSnapshot {
            current_phase: "uninitialized".to_string(),
            attempt_count: 0,
            success_count: 0,
            consecutive_failures: 0,
            last_connect_error: None,
            last_connect_error_at: None,
            last_successful_connect_at: None,
            last_attempt_at: None,
            last_connect_url: None,
        }
    }
}

/// Strip `?token=…` and `?jwt=…` query strings so auth material never appears
/// in the `/debug/ws-state` response. Keeps the path so the host is still
/// diagnosable.
fn redact_url(url: &str) -> String {
    match url.find('?') {
        Some(pos) => format!("{}?<redacted>", &url[..pos]),
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_strips_query_string() {
        assert_eq!(redact_url("ws://1.2.3.4:8080/ws/agent"), "ws://1.2.3.4:8080/ws/agent");
        assert_eq!(
            redact_url("ws://1.2.3.4:8080/ws/agent?token=SECRET"),
            "ws://1.2.3.4:8080/ws/agent?<redacted>"
        );
        assert_eq!(
            redact_url("ws://1.2.3.4:8080/ws/agent?jwt=eyJhbGc..."),
            "ws://1.2.3.4:8080/ws/agent?<redacted>"
        );
    }

    /// All mutating behavior in one test: the WS_STATE OnceLock is global and
    /// tokio::test runs in parallel, so asserting against separate tests would
    /// race. Verify the full state-machine in sequence inside a single test.
    #[tokio::test]
    async fn ws_state_lifecycle_behaves_correctly() {
        init();

        // Snapshot survives with or without prior state — must not panic.
        let _ = snapshot().await;

        // Two failures increment consecutive_failures monotonically.
        record_failure("boom").await;
        record_failure("boom2").await;
        let before = snapshot().await;
        assert!(before.consecutive_failures >= 2, "consecutive_failures={}", before.consecutive_failures);
        assert_eq!(before.current_phase, "disconnected");

        // Attempt + success resets consecutive_failures, stamps timestamps,
        // redacts the URL query string.
        record_attempt("ws://1.2.3.4:8080/ws/agent?token=SECRET").await;
        record_success().await;
        let after = snapshot().await;
        assert_eq!(after.consecutive_failures, 0);
        assert_eq!(after.current_phase, "connected");
        assert!(after.last_successful_connect_at.is_some());
        assert_eq!(
            after.last_connect_url.as_deref(),
            Some("ws://1.2.3.4:8080/ws/agent?<redacted>"),
            "token query-string must be redacted from the diagnostic endpoint"
        );

        // Long errors truncate to the 500-char cap so the endpoint payload
        // stays bounded.
        let long = "x".repeat(1000);
        record_failure(&long).await;
        let s = snapshot().await;
        let err = s.last_connect_error.expect("last_connect_error recorded");
        assert_eq!(err.chars().count(), 500, "error must be truncated to 500 chars");
    }
}
