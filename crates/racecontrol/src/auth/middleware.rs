use std::sync::Arc;

use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ─── Staff JWT Claims ─────────────────────────────────────────────────────

/// JWT claims for staff/admin authentication.
/// Separate from customer `Claims` -- staff tokens carry a `role` field
/// that customer tokens do not have. `require_staff_jwt` rejects tokens
/// that cannot be deserialized into StaffClaims (missing role field).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffClaims {
    /// Staff identifier ("admin", employee ID, etc.)
    pub sub: String,
    /// Must be "staff" -- middleware rejects any other value
    pub role: String,
    /// Expiration (UNIX timestamp)
    pub exp: usize,
    /// Issued-at (UNIX timestamp)
    pub iat: usize,
}

// ─── Token extraction helper ──────────────────────────────────────────────

/// Extract and validate StaffClaims from the request Authorization header.
/// Returns Ok(StaffClaims) on success, Err(()) on any failure.
fn extract_staff_claims<B>(state: &Arc<AppState>, req: &Request<B>) -> Result<StaffClaims, ()> {
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(())?;

    let token = auth_header.strip_prefix("Bearer ").ok_or(())?;

    let secret = &state.config.auth.jwt_secret;

    // MMA-P3: Try current secret first, then previous secret for rotation grace period.
    // This prevents instant staff lockout when jwt_secret changes in config.
    let data = jsonwebtoken::decode::<StaffClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .or_else(|_| {
        // Fall back to previous secret if configured
        if let Some(ref prev_secret) = state.config.auth.jwt_secret_previous {
            if !prev_secret.is_empty() {
                return jsonwebtoken::decode::<StaffClaims>(
                    token,
                    &DecodingKey::from_secret(prev_secret.as_bytes()),
                    &Validation::default(),
                );
            }
        }
        Err(jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidToken))
    })
    .map_err(|_| ())?;

    // Verify the role is "staff"
    if data.claims.role != "staff" {
        return Err(());
    }

    Ok(data.claims)
}

// ─── Strict middleware (rejects unauthenticated requests) ─────────────────

/// Axum middleware that enforces staff JWT authentication.
///
/// - Extracts `Authorization: Bearer {token}` header
/// - Decodes with `jsonwebtoken::decode::<StaffClaims>` using `config.auth.jwt_secret`
/// - On success: inserts `StaffClaims` into request extensions, calls next
/// - On failure: returns 401 UNAUTHORIZED
pub async fn require_staff_jwt(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let (mut parts, body) = req.into_parts();

    // Try to build a temporary request for extraction
    let temp_req = Request::from_parts(parts.clone(), axum::body::Body::empty());
    match extract_staff_claims(&state, &temp_req) {
        Ok(claims) => {
            parts.extensions.insert(claims);
            let req = Request::from_parts(parts, body);
            Ok(next.run(req).await)
        }
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}

// ─── Permissive middleware (logs warnings but allows through) ─────────────

/// Permissive variant of staff JWT middleware for the expand-migrate-contract
/// rollout. Logs warnings for unauthenticated staff requests but always
/// allows the request through. Once dashboard and bots send JWTs, switch
/// to `require_staff_jwt` (strict).
pub async fn require_staff_jwt_permissive(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let (mut parts, body) = req.into_parts();

    let temp_req = Request::from_parts(parts.clone(), axum::body::Body::empty());
    match extract_staff_claims(&state, &temp_req) {
        Ok(claims) => {
            parts.extensions.insert(claims);
        }
        Err(_) => {
            tracing::warn!(
                path = %parts.uri.path(),
                method = %parts.method,
                "Unauthenticated staff request (permissive mode)"
            );
        }
    }

    let req = Request::from_parts(parts, body);
    next.run(req).await
}

// ─── Create staff JWT ─────────────────────────────────────────────────────

/// Create a staff JWT token with the given staff_id and duration.
///
/// Returns the encoded JWT string on success.
pub fn create_staff_jwt(secret: &str, staff_id: &str, duration_hours: u64) -> Result<String, String> {
    let now = chrono::Utc::now();
    let exp = now + chrono::Duration::hours(duration_hours as i64);

    let claims = StaffClaims {
        sub: staff_id.to_string(),
        role: "staff".to_string(),
        iat: now.timestamp() as usize,
        exp: exp.timestamp() as usize,
    };

    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("JWT encode error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware::from_fn_with_state,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    const TEST_SECRET: &str = "test-secret-key-for-unit-tests-only";

    /// Build a minimal AppState with a known JWT secret for testing.
    async fn test_state() -> Arc<AppState> {
        let mut config = crate::config::Config::default_test();
        config.auth.jwt_secret = TEST_SECRET.to_string();

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool");

        let field_cipher = crate::crypto::encryption::test_field_cipher();
        Arc::new(AppState::new(config, pool, field_cipher))
    }

    /// Build a test router: a single GET /test behind require_staff_jwt middleware.
    fn test_router(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(from_fn_with_state(state.clone(), require_staff_jwt))
            .with_state(state)
    }

    fn make_request(token: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().uri("/test").method("GET");
        if let Some(t) = token {
            builder = builder.header("Authorization", format!("Bearer {}", t));
        }
        builder.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn middleware_rejects_no_auth_header() {
        let state = test_state().await;
        let app = test_router(state);
        let resp = app.oneshot(make_request(None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_rejects_invalid_jwt() {
        let state = test_state().await;
        let app = test_router(state);
        let resp = app.oneshot(make_request(Some("not-a-valid-jwt"))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_rejects_expired_jwt() {
        let state = test_state().await;
        // Create a token that expired 1 hour ago
        let now = chrono::Utc::now();
        let claims = StaffClaims {
            sub: "admin".to_string(),
            role: "staff".to_string(),
            iat: (now - chrono::Duration::hours(2)).timestamp() as usize,
            exp: (now - chrono::Duration::hours(1)).timestamp() as usize,
        };
        let token = jsonwebtoken::encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();

        let app = test_router(state);
        let resp = app.oneshot(make_request(Some(&token))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_accepts_valid_staff_jwt() {
        let state = test_state().await;
        let token = create_staff_jwt(TEST_SECRET, "admin", 24).unwrap();
        let app = test_router(state);
        let resp = app.oneshot(make_request(Some(&token))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn create_staff_jwt_roundtrip() {
        let token = create_staff_jwt(TEST_SECRET, "staff_42", 8).unwrap();
        let data = jsonwebtoken::decode::<StaffClaims>(
            &token,
            &DecodingKey::from_secret(TEST_SECRET.as_bytes()),
            &Validation::default(),
        )
        .unwrap();
        assert_eq!(data.claims.sub, "staff_42");
        assert_eq!(data.claims.role, "staff");
    }

    #[tokio::test]
    async fn middleware_rejects_customer_jwt() {
        let state = test_state().await;
        // Create a customer JWT (Claims struct -- no role field)
        let customer_token = crate::auth::create_jwt("driver_123", TEST_SECRET).unwrap();
        let app = test_router(state);
        let resp = app.oneshot(make_request(Some(&customer_token))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_rejects_wrong_role() {
        let state = test_state().await;
        // Create a token with role="customer" instead of "staff"
        let now = chrono::Utc::now();
        let claims = StaffClaims {
            sub: "someone".to_string(),
            role: "customer".to_string(),
            iat: now.timestamp() as usize,
            exp: (now + chrono::Duration::hours(1)).timestamp() as usize,
        };
        let token = jsonwebtoken::encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();

        let app = test_router(state);
        let resp = app.oneshot(make_request(Some(&token))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
