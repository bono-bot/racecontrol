//! M1-SEC: Service key authentication middleware for rc-sentry-ai.
//!
//! Protects all endpoints except /health and /api/v1/privacy/consent.
//! Service key must be set in rc-sentry-ai.toml under [service].service_key.
//! Clients pass it as X-Service-Key header.

use std::sync::Arc;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};

/// Public paths that don't require authentication.
const PUBLIC_PATHS: &[&str] = &["/health", "/api/v1/privacy/consent"];

/// Service key config — passed via State, not Extension (avoids layer ordering bugs).
#[derive(Clone)]
pub struct ServiceKeyConfig {
    pub key: Option<String>,
}

/// Middleware that checks X-Service-Key header against configured service key.
/// Uses from_fn_with_state to capture config via State extractor (not Extension).
/// MMA fix: Extension-based config had silent bypass if layers ordered wrong.
pub async fn require_service_key(
    State(config): State<Arc<ServiceKeyConfig>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();

    // Public paths are always allowed
    if PUBLIC_PATHS.iter().any(|p| path == *p || path.starts_with(p)) {
        return Ok(next.run(req).await);
    }

    match &config.key {
        Some(expected) => {
            let provided = req.headers().get("x-service-key").and_then(|v| v.to_str().ok());

            match provided {
                Some(k) if k == expected.as_str() => Ok(next.run(req).await),
                Some(_) => {
                    tracing::warn!(path = %path, "rc-sentry-ai: invalid service key");
                    Err(StatusCode::UNAUTHORIZED)
                }
                None => {
                    tracing::warn!(path = %path, "rc-sentry-ai: missing X-Service-Key header");
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        }
        None => {
            tracing::warn!(
                path = %path,
                "rc-sentry-ai: no service_key configured — allowing unauthenticated request"
            );
            Ok(next.run(req).await)
        }
    }
}
