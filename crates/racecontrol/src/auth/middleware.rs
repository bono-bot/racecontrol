use std::sync::Arc;

use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::state::AppState;

// ─── Role constants ───────────────────────────────────────────────────────────

/// Valid staff roles in ascending privilege order.
/// "staff" is a legacy alias accepted during token decode but mapped to "cashier".
pub const ROLE_CASHIER: &str = "cashier";
pub const ROLE_MANAGER: &str = "manager";
pub const ROLE_SUPERADMIN: &str = "superadmin";

/// All valid roles that require_staff_jwt will accept.
const VALID_ROLES: &[&str] = &["cashier", "manager", "superadmin", "staff"];

// ─── Staff JWT Claims ─────────────────────────────────────────────────────

/// JWT claims for staff/admin authentication.
/// Separate from customer `Claims` -- staff tokens carry a `role` field
/// that customer tokens do not have. `require_staff_jwt` rejects tokens
/// that cannot be deserialized into StaffClaims (missing role field).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffClaims {
    /// Staff identifier ("admin", employee ID, etc.)
    pub sub: String,
    /// Role: "cashier", "manager", "superadmin" (or legacy "staff" -> treated as "cashier")
    pub role: String,
    /// Expiration (UNIX timestamp)
    pub exp: usize,
    /// Issued-at (UNIX timestamp)
    pub iat: usize,
}

impl StaffClaims {
    /// Normalized role — maps legacy "staff" to "cashier" for uniform comparison.
    pub fn normalized_role(&self) -> &str {
        if self.role == "staff" {
            "cashier"
        } else {
            &self.role
        }
    }

    /// Returns true if this claims' role is in the given allowed roles list.
    /// Applies legacy "staff" -> "cashier" mapping.
    pub fn has_role(&self, allowed: &[&str]) -> bool {
        allowed.contains(&self.normalized_role())
    }
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

    // Accept "cashier", "manager", "superadmin" and legacy "staff"
    if !VALID_ROLES.contains(&data.claims.role.as_str()) {
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

// SEC-FP-4: Permissive JWT middleware DELETED (was dead code, regression risk).
// All routes now use strict require_staff_jwt only.

// ─── RBAC role-checking middleware ────────────────────────────────────────

/// Middleware that enforces RBAC role gating AFTER require_staff_jwt has
/// already inserted StaffClaims into request extensions.
///
/// `allowed_roles` is a `&'static [&'static str]` of permitted roles, e.g.
/// `&["manager", "superadmin"]`.
///
/// Returns 403 with `{"error": "Insufficient permissions"}` if the claims
/// role is not in the allowed list.
///
/// Must be used AFTER `require_staff_jwt` in the middleware chain -- it
/// expects StaffClaims to be present in extensions.
pub async fn require_role(
    allowed_roles: &'static [&'static str],
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    match req.extensions().get::<StaffClaims>() {
        Some(claims) if claims.has_role(allowed_roles) => Ok(next.run(req).await),
        Some(claims) => {
            tracing::warn!(
                role = %claims.role,
                allowed = ?allowed_roles,
                path = %req.uri().path(),
                "RBAC: role insufficient"
            );
            Err(
                (
                    StatusCode::FORBIDDEN,
                    Json(json!({ "error": "Insufficient permissions" })),
                )
                    .into_response(),
            )
        }
        None => {
            // No claims in extensions — require_staff_jwt wasn't in the chain
            tracing::error!(
                path = %req.uri().path(),
                "require_role called without require_staff_jwt in middleware chain"
            );
            Err(
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({ "error": "Not authenticated" })),
                )
                    .into_response(),
            )
        }
    }
}

/// Axum middleware layer that requires manager+ role.
/// Use: `.layer(axum::middleware::from_fn(require_role_manager))`
pub async fn require_role_manager(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    require_role(&["manager", "superadmin"], req, next).await
}

/// Axum middleware layer that requires superadmin role.
/// Use: `.layer(axum::middleware::from_fn(require_role_superadmin))`
pub async fn require_role_superadmin(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    require_role(&["superadmin"], req, next).await
}

// ─── Create staff JWT ─────────────────────────────────────────────────────

/// Create a staff JWT token with the given staff_id and duration.
///
/// Issues "cashier" role (the minimum privilege level) for backward compatibility.
/// Use `create_staff_jwt_with_role` when the caller knows the specific role.
///
/// Returns the encoded JWT string on success.
pub fn create_staff_jwt(secret: &str, staff_id: &str, duration_hours: u64) -> Result<String, String> {
    create_staff_jwt_with_role(secret, staff_id, "cashier", duration_hours)
}

/// Create a staff JWT token with an explicit role.
///
/// Role must be one of: "cashier", "manager", "superadmin" (or legacy "staff").
pub fn create_staff_jwt_with_role(
    secret: &str,
    staff_id: &str,
    role: &str,
    duration_hours: u64,
) -> Result<String, String> {
    let now = chrono::Utc::now();
    let exp = now + chrono::Duration::hours(duration_hours as i64);

    let claims = StaffClaims {
        sub: staff_id.to_string(),
        role: role.to_string(),
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


// ─── Pod JWT Claims (Phase 306) ──────────────────────────────────────────────

/// JWT claims for pod/agent WebSocket authentication.
///
/// Issued by the server after PSK bootstrap authentication (WSAUTH-01/04).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PodClaims {
    /// Canonical pod ID, e.g. "pod_3"
    pub pod_id: String,
    /// Pod number (1-based), e.g. 3
    pub pod_number: u32,
    /// Expiration (UNIX timestamp)
    pub exp: usize,
    /// Issued-at (UNIX timestamp)
    pub iat: usize,
}

/// Create a JWT for pod WebSocket authentication (Phase 306).
pub fn create_pod_jwt(
    secret: &str,
    pod_id: &str,
    pod_number: u32,
    duration_hours: u64,
) -> Result<String, String> {
    let now = chrono::Utc::now();
    let exp = now + chrono::Duration::hours(duration_hours as i64);
    let claims = PodClaims {
        pod_id: pod_id.to_string(),
        pod_number,
        iat: now.timestamp() as usize,
        exp: exp.timestamp() as usize,
    };
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("Pod JWT encode error: {}", e))
}

/// Decode and validate a pod JWT (Phase 306).
/// Tries current secret first, then previous secret (rotation grace).
pub fn decode_pod_jwt(
    token: &str,
    secret: &str,
    prev_secret: Option<&str>,
) -> Result<PodClaims, String> {
    let result = jsonwebtoken::decode::<PodClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    );
    match result {
        Ok(data) => Ok(data.claims),
        Err(primary_err) => {
            if let Some(prev) = prev_secret {
                if !prev.is_empty() {
                    return jsonwebtoken::decode::<PodClaims>(
                        token,
                        &DecodingKey::from_secret(prev.as_bytes()),
                        &Validation::default(),
                    )
                    .map(|d| d.claims)
                    .map_err(|_| format!("Pod JWT decode failed (both secrets): {}", primary_err));
                }
            }
            Err(format!("Pod JWT decode failed: {}", primary_err))
        }
    }
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
            role: "cashier".to_string(),
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
    async fn middleware_accepts_cashier_jwt() {
        let state = test_state().await;
        let token = create_staff_jwt_with_role(TEST_SECRET, "staff_1", "cashier", 24).unwrap();
        let app = test_router(state);
        let resp = app.oneshot(make_request(Some(&token))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_accepts_valid_staff_jwt() {
        // Keep backward compat alias test
        let state = test_state().await;
        let token = create_staff_jwt(TEST_SECRET, "admin", 24).unwrap();
        let app = test_router(state);
        let resp = app.oneshot(make_request(Some(&token))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_accepts_manager_jwt() {
        let state = test_state().await;
        let token = create_staff_jwt_with_role(TEST_SECRET, "staff_2", "manager", 24).unwrap();
        let app = test_router(state);
        let resp = app.oneshot(make_request(Some(&token))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_accepts_superadmin_jwt() {
        let state = test_state().await;
        let token = create_staff_jwt_with_role(TEST_SECRET, "admin", "superadmin", 24).unwrap();
        let app = test_router(state);
        let resp = app.oneshot(make_request(Some(&token))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_accepts_legacy_staff_role() {
        // Backward compat: tokens with role="staff" should still be accepted
        let state = test_state().await;
        let now = chrono::Utc::now();
        let claims = StaffClaims {
            sub: "legacy".to_string(),
            role: "staff".to_string(),
            iat: now.timestamp() as usize,
            exp: (now + chrono::Duration::hours(24)).timestamp() as usize,
        };
        let legacy_token = jsonwebtoken::encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();
        let app = test_router(state);
        let resp = app.oneshot(make_request(Some(&legacy_token))).await.unwrap();
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
        // create_staff_jwt now issues "cashier" role
        assert_eq!(data.claims.role, "cashier");
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
        // Create a token with role="customer" (not in valid roles list)
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

    // ─── RBAC role-checking unit tests ────────────────────────────────────────

    #[test]
    fn cashier_role_normalized_is_cashier() {
        let claims = StaffClaims { sub: "x".to_string(), role: "cashier".to_string(), iat: 0, exp: 9999999999 };
        assert_eq!(claims.normalized_role(), "cashier");
    }

    #[test]
    fn legacy_staff_role_normalized_is_cashier() {
        let claims = StaffClaims { sub: "x".to_string(), role: "staff".to_string(), iat: 0, exp: 9999999999 };
        assert_eq!(claims.normalized_role(), "cashier");
    }

    #[test]
    fn manager_has_role_manager() {
        let claims = StaffClaims { sub: "x".to_string(), role: "manager".to_string(), iat: 0, exp: 9999999999 };
        assert!(claims.has_role(&["manager", "superadmin"]));
        assert!(!claims.has_role(&["superadmin"]));
        assert!(claims.has_role(&["cashier", "manager", "superadmin"]));
    }

    #[test]
    fn superadmin_has_all_roles() {
        let claims = StaffClaims { sub: "x".to_string(), role: "superadmin".to_string(), iat: 0, exp: 9999999999 };
        assert!(claims.has_role(&["superadmin"]));
        assert!(claims.has_role(&["manager", "superadmin"]));
        assert!(claims.has_role(&["cashier", "manager", "superadmin"]));
    }

    #[test]
    fn cashier_blocked_from_manager_routes() {
        let claims = StaffClaims { sub: "x".to_string(), role: "cashier".to_string(), iat: 0, exp: 9999999999 };
        assert!(!claims.has_role(&["manager", "superadmin"]));
        assert!(!claims.has_role(&["superadmin"]));
    }

    // ─── Role middleware integration tests ────────────────────────────────────

    fn test_router_with_role(state: Arc<AppState>, allowed: &'static [&'static str]) -> Router {
        Router::new()
            .route("/protected", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(move |req, next| {
                require_role(allowed, req, next)
            }))
            .layer(from_fn_with_state(state.clone(), require_staff_jwt))
            .with_state(state)
    }

    fn make_authed_request(token: &str) -> Request<Body> {
        Request::builder()
            .uri("/protected")
            .method("GET")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn cashier_blocked_from_manager_endpoint() {
        let state = test_state().await;
        let token = create_staff_jwt_with_role(TEST_SECRET, "cashier_1", "cashier", 24).unwrap();
        let app = test_router_with_role(state, &["manager", "superadmin"]);
        let resp = app.oneshot(make_authed_request(&token)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn manager_allowed_on_manager_endpoint() {
        let state = test_state().await;
        let token = create_staff_jwt_with_role(TEST_SECRET, "manager_1", "manager", 24).unwrap();
        let app = test_router_with_role(state, &["manager", "superadmin"]);
        let resp = app.oneshot(make_authed_request(&token)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn manager_blocked_from_superadmin_endpoint() {
        let state = test_state().await;
        let token = create_staff_jwt_with_role(TEST_SECRET, "manager_1", "manager", 24).unwrap();
        let app = test_router_with_role(state, &["superadmin"]);
        let resp = app.oneshot(make_authed_request(&token)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn superadmin_allowed_everywhere() {
        let state = test_state().await;
        let token = create_staff_jwt_with_role(TEST_SECRET, "admin", "superadmin", 24).unwrap();
        // manager route
        let app_m = test_router_with_role(state.clone(), &["manager", "superadmin"]);
        let resp = app_m.oneshot(make_authed_request(&token)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let token2 = create_staff_jwt_with_role(TEST_SECRET, "admin", "superadmin", 24).unwrap();
        // superadmin route
        let app_s = test_router_with_role(state, &["superadmin"]);
        let resp2 = app_s.oneshot(make_authed_request(&token2)).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);
    }
}
