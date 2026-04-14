use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tokio::time::Duration;

use crate::state::AppState;

/// WS-HARDEN: Track failed WS auth attempts per source. 5 failures in 5min = lockout.
static WS_AUTH_FAILURES: std::sync::LazyLock<Mutex<HashMap<String, (u32, Instant)>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Check if a WS source is locked out from auth failures. Returns true if locked out.
pub(crate) fn ws_auth_locked_out(source: &str) -> bool {
    let map = WS_AUTH_FAILURES.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((count, first_failure)) = map.get(source) {
        if first_failure.elapsed() < Duration::from_secs(300) && *count >= 5 {
            return true;
        }
    }
    false
}

/// Record a failed WS auth attempt.
pub(crate) fn ws_auth_record_failure(source: &str) {
    let mut map = WS_AUTH_FAILURES.lock().unwrap_or_else(|p| p.into_inner());
    let entry = map.entry(source.to_string()).or_insert((0, Instant::now()));
    if entry.1.elapsed() > Duration::from_secs(300) {
        // Reset window
        *entry = (1, Instant::now());
    } else {
        entry.0 += 1;
    }
}

/// Query parameters for WS authentication
#[derive(serde::Deserialize, Default)]
pub struct WsAuthParams {
    /// PSK bootstrap token — must match config.cloud.terminal_secret
    #[serde(default)]
    pub token: Option<String>,
    /// Per-pod JWT token — issued by server after first PSK auth (Phase 306)
    #[serde(default)]
    pub jwt: Option<String>,
}

/// WS authentication result for the agent endpoint (Phase 306).
pub(crate) enum AgentAuthResult {
    PskAuthenticated,
    JwtAuthenticated { pod_id: String, pod_number: u32 },
}

/// Validate WebSocket token against terminal_secret (if configured).
/// Returns true if: no secret configured (dev mode), or token matches.
pub(crate) fn verify_ws_token(state: &AppState, token: &Option<String>) -> bool {
    match &state.config.cloud.terminal_secret {
        None => true, // dev mode — no auth required
        Some(secret) if secret.is_empty() => true,
        Some(secret) => token.as_deref() == Some(secret.as_str()),
    }
}

/// Phase 306: Authenticate a pod WS connection.
/// Tries JWT first (steady-state), then PSK (bootstrap).
pub(crate) fn authenticate_agent_ws(state: &AppState, params: &WsAuthParams) -> Result<AgentAuthResult, String> {
    if let Some(ref jwt_token) = params.jwt {
        if !jwt_token.is_empty() {
            let prev_secret = state.config.auth.jwt_secret_previous.as_deref();
            match crate::auth::middleware::decode_pod_jwt(
                jwt_token,
                &state.config.auth.jwt_secret,
                prev_secret,
            ) {
                Ok(claims) => {
                    return Ok(AgentAuthResult::JwtAuthenticated {
                        pod_id: claims.pod_id,
                        pod_number: claims.pod_number,
                    });
                }
                Err(e) => return Err(format!("Invalid pod JWT: {}", e)),
            }
        }
    }
    let psk_ok = match &state.config.cloud.terminal_secret {
        None => true,
        Some(s) if s.is_empty() => true,
        Some(secret) => {
            let token_match = params.token.as_deref() == Some(secret.as_str());
            if !token_match && params.token.is_none() {
                // No token provided at all — allow with warning (backward compat).
                // Agent will identify via Register message. Agents without ws_secret
                // in their config still need to connect for fleet operations.
                tracing::warn!(
                    "WS agent connection with no PSK token — allowing for backward compatibility. \
                     Configure ws_secret in rc-agent.toml [core] section to suppress this warning."
                );
                true
            } else {
                token_match
            }
        }
    };
    if psk_ok { Ok(AgentAuthResult::PskAuthenticated) }
    else { Err("Invalid or missing PSK token".to_string()) }
}

/// Phase 306: Issue a 24-hour pod JWT and queue it for sending.
pub(crate) fn issue_pod_jwt_to_agent(
    state: &AppState,
    pod_id: &str,
    pod_number: u32,
    cmd_tx: &tokio::sync::mpsc::Sender<rc_common::protocol::CoreMessage>,
) {
    use rc_common::protocol::{CoreMessage, CoreToAgentMessage};
    match crate::auth::middleware::create_pod_jwt(&state.config.auth.jwt_secret, pod_id, pod_number, 24) {
        Ok(token) => {
            let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp();
            if cmd_tx.try_send(CoreMessage::wrap(CoreToAgentMessage::IssueJwt { token, expires_at })).is_ok() {
                tracing::info!("Phase 306: JWT issued to pod {} (expires_at={})", pod_id, expires_at);
            } else {
                tracing::warn!("Phase 306: Failed to queue IssueJwt for pod {}", pod_id);
            }
        }
        Err(e) => tracing::error!("Phase 306: Failed to create pod JWT for {}: {}", pod_id, e),
    }
}
