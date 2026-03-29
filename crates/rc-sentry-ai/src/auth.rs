//! M1-SEC: Service key authentication middleware for rc-sentry-ai.
//!
//! Protects all endpoints except /health and /api/v1/privacy/consent.
//! Service key must be set in rc-sentry-ai.toml under [service].service_key.
//! Clients pass it as X-Service-Key header.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

/// Public paths that don't require authentication.
const PUBLIC_PATHS: &[&str] = &["/health", "/api/v1/privacy/consent"];

/// Middleware that checks X-Service-Key header against configured service key.
/// If no service_key is configured, all requests are allowed (backwards compat warning).
pub async fn require_service_key(
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();

    // Public paths are always allowed
    if PUBLIC_PATHS.iter().any(|p| path == *p || path.starts_with(p)) {
        return Ok(next.run(req).await);
    }

    // Extract configured service key from request extensions
    let expected_key = req.extensions().get::<ServiceKeyConfig>();

    match expected_key {
        Some(config) if config.key.is_some() => {
            // Safety: is_some() check above guarantees unwrap won't panic
            let expected = match config.key.as_ref() {
                Some(k) => k,
                None => return Err(StatusCode::INTERNAL_SERVER_ERROR),
            };
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
        _ => {
            // No service key configured — warn but allow (first-run compatibility)
            tracing::warn!(
                path = %path,
                "rc-sentry-ai: no service_key configured — allowing unauthenticated request"
            );
            Ok(next.run(req).await)
        }
    }
}

/// Extension type to pass service key config into middleware.
#[derive(Clone)]
pub struct ServiceKeyConfig {
    pub key: Option<String>,
}
