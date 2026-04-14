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
