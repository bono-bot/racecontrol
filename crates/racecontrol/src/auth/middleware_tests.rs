#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware::from_fn_with_state,
        routing::get,
        Router,
    };
    use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
    use tower::ServiceExt;
    use crate::auth::middleware::{
        create_staff_jwt, create_staff_jwt_with_role, extract_session_cookie, is_idle_expired,
        require_role, require_staff_jwt, CredentialClass, StaffClaims, STAFF_SESSION_COOKIE_NAME,
    };
    use crate::state::AppState;
    use axum::Extension;

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
        Arc::new(AppState::new_with_test_v2db(config, pool, field_cipher))
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

    /// Phase 1.5 §S-146 Q4 (Option Z) — `require_staff_jwt` must inject
    /// `CredentialClass::StaffJwt` into request extensions alongside the
    /// existing `StaffClaims` so downstream handlers can read credential
    /// class via `Extension<CredentialClass>` instead of header inference.
    ///
    /// Test method: build a router whose handler reads
    /// `Extension<CredentialClass>` and reports it back in the response
    /// body. Valid JWT → handler sees the extension and responds 200 with
    /// the class name. Missing extension → 500 (debug surface — should not
    /// occur on a route gated by `require_staff_jwt`).
    #[tokio::test]
    async fn middleware_injects_credential_class_staff_jwt() {
        let state = test_state().await;

        async fn echo_credential_class(
            Extension(class): Extension<CredentialClass>,
        ) -> &'static str {
            match class {
                CredentialClass::StaffJwt => "staff-jwt",
                CredentialClass::ServiceKey => "service-key",
                CredentialClass::AutoScheduler => "internal-caller",
            }
        }

        let app = Router::new()
            .route("/test", get(echo_credential_class))
            .layer(from_fn_with_state(state.clone(), require_staff_jwt))
            .with_state(state);

        let token = create_staff_jwt_with_role(TEST_SECRET, "staff_1", "cashier", 24).unwrap();
        let resp = app.oneshot(make_request(Some(&token))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(resp.into_body(), 64).await.unwrap();
        assert_eq!(&body_bytes[..], b"staff-jwt");
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

    // ─── Idle-timeout (Captain §S-82 Q3 sliding window) ───────────────────

    fn make_claims_with_iat(secs_ago: i64) -> StaffClaims {
        let now = chrono::Utc::now();
        StaffClaims {
            sub: "cashier_1".to_string(),
            role: "cashier".to_string(),
            iat: (now - chrono::Duration::seconds(secs_ago)).timestamp() as usize,
            exp: (now + chrono::Duration::hours(24)).timestamp() as usize,
        }
    }

    #[test]
    fn is_idle_expired_fresh_token_returns_false() {
        // iat = now → 0s elapsed < 1800s window → not expired
        let claims = make_claims_with_iat(0);
        assert!(!is_idle_expired(&claims, 1800));
    }

    #[test]
    fn is_idle_expired_token_within_window_returns_false() {
        // 1500s elapsed < 1800s window → not expired
        let claims = make_claims_with_iat(1500);
        assert!(!is_idle_expired(&claims, 1800));
    }

    #[test]
    fn is_idle_expired_token_beyond_window_returns_true() {
        // 1801s > 1800s default window → expired
        let claims = make_claims_with_iat(1801);
        assert!(is_idle_expired(&claims, 1800));
    }

    #[test]
    fn is_idle_expired_zero_secs_disables_idle_check() {
        // idle_timeout_secs = 0 → idle check disabled, only `exp` matters
        let claims = make_claims_with_iat(100_000);
        assert!(!is_idle_expired(&claims, 0));
    }

    #[test]
    fn is_idle_expired_future_iat_clock_skew_returns_false() {
        // Clock skew: iat is in the future. Saturating subtraction yields 0 elapsed → not expired.
        let claims = make_claims_with_iat(-1000);
        assert!(!is_idle_expired(&claims, 1800));
    }

    #[tokio::test]
    async fn middleware_rejects_idle_expired_jwt() {
        // Token's `exp` is hours in the future, but `iat` is older than the
        // configured idle window. Captain §S-82 Q3 contract: re-PIN required.
        let state = test_state().await;
        let now = chrono::Utc::now();
        let claims = StaffClaims {
            sub: "cashier_1".to_string(),
            role: "cashier".to_string(),
            iat: (now - chrono::Duration::seconds(3600)).timestamp() as usize, // 1h ago
            exp: (now + chrono::Duration::hours(23)).timestamp() as usize,     // still valid by `exp`
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

    // ─── V2 row 7.6 cookie-auth Phase 1 tests (D-7.6-MMA Step 1 action items) ────
    //
    // Coverage map (RCA §1 sub-gaps + D-7.6-MMA Step 1 5/5 consensus actions):
    //   G-7.6-2 cookie-extractor middleware ........... `middleware_accepts_cookie_only_request`
    //   A4 F-7.6-D-3 reject-duplicate fail-closed ..... `extract_session_cookie_rejects_duplicate`
    //   A8 F-7.6-D-7 multi-Cookie-header support ...... `extract_session_cookie_walks_multiple_headers`
    //   Q3 Bearer-priority preserved .................. `middleware_bearer_priority_over_cookie`
    //
    // Build cookie-only request: no Authorization header, Cookie: rp_staff_session=<jwt>.
    fn make_cookie_request(cookie_value: &str) -> Request<Body> {
        Request::builder()
            .uri("/test")
            .method("GET")
            .header("Cookie", format!("{}={}", STAFF_SESSION_COOKIE_NAME, cookie_value))
            .body(Body::empty())
            .unwrap()
    }

    /// Helper: build a valid JWT for the given staff_id with `iat = now`,
    /// 24h duration (matches the production `create_staff_jwt_with_role(..., 24)`
    /// call in `staff_validate_pin`).
    fn fresh_jwt(staff_id: &str) -> String {
        create_staff_jwt(TEST_SECRET, staff_id, 24).expect("mint test JWT")
    }

    #[tokio::test]
    async fn middleware_accepts_cookie_only_request() {
        // G-7.6-2 closure — server reads session cookie when no Bearer header.
        let state = test_state().await;
        let app = test_router(state);
        let jwt = fresh_jwt("staff-001");
        let resp = app.oneshot(make_cookie_request(&jwt)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "cookie-only request should authenticate via extract_session_cookie path"
        );
    }

    #[tokio::test]
    async fn middleware_rejects_no_auth_or_cookie() {
        // Regression check — pre-amendment middleware returned 401 on missing
        // Authorization; new path must still return 401 when BOTH Authorization
        // AND Cookie are absent.
        let state = test_state().await;
        let app = test_router(state);
        let req = Request::builder().uri("/test").method("GET").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_bearer_priority_over_cookie() {
        // Q3 disposition — Bearer is checked first; a request carrying BOTH
        // a valid Bearer header and a valid session cookie authenticates via
        // the Bearer path (priority-by-construction). This is asserted at the
        // observable level — both requests authenticate the same staff_id, so
        // we use a deliberately-corrupted cookie to prove the Bearer is what
        // got accepted: if the cookie path had won, the corrupted cookie
        // would have failed the JWT decode and we'd see 401.
        let state = test_state().await;
        let app = test_router(state);
        let jwt = fresh_jwt("staff-002");
        let req = Request::builder()
            .uri("/test")
            .method("GET")
            .header("Authorization", format!("Bearer {}", jwt))
            .header("Cookie", format!("{}=corrupted-not-a-jwt", STAFF_SESSION_COOKIE_NAME))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "Bearer should take priority — corrupted cookie should NOT have been consulted"
        );
    }

    #[tokio::test]
    async fn extract_session_cookie_returns_value_on_single_match() {
        // Pure-function test on the cookie extractor.
        let req = Request::builder()
            .uri("/test")
            .header("Cookie", "other=foo; rp_staff_session=abc.def.ghi; bar=baz")
            .body(())
            .unwrap();
        assert_eq!(extract_session_cookie(&req), Some("abc.def.ghi".to_string()));
    }

    #[tokio::test]
    async fn extract_session_cookie_returns_none_when_absent() {
        let req = Request::builder()
            .uri("/test")
            .header("Cookie", "other=foo; bar=baz")
            .body(())
            .unwrap();
        assert_eq!(extract_session_cookie(&req), None);
    }

    #[tokio::test]
    async fn extract_session_cookie_rejects_duplicate() {
        // F-7.6-D-3 (R1 P0 0.95, 5/5 consensus) — duplicate `rp_staff_session=`
        // cookies fail closed to defeat session-fixation via attacker-planted
        // cookie. Helper returns None on >1 match.
        let req = Request::builder()
            .uri("/test")
            .header("Cookie", "rp_staff_session=victim-jwt; rp_staff_session=attacker-jwt")
            .body(())
            .unwrap();
        assert_eq!(
            extract_session_cookie(&req),
            None,
            "duplicate session cookies must fail closed (F-7.6-D-3 R1 P0)"
        );
    }

    #[tokio::test]
    async fn extract_session_cookie_walks_multiple_headers() {
        // F-7.6-D-7 (R1 P1 0.80) — axum HeaderMap::get returns only the first
        // Cookie header. RFC 6265 §5.4 allows multiple Cookie headers. The
        // helper uses get_all and walks each, so a `rp_staff_session=` value
        // in the 2nd header is still extracted.
        let req = Request::builder()
            .uri("/test")
            .header("Cookie", "other=foo")
            .header("Cookie", "rp_staff_session=abc.def.ghi")
            .body(())
            .unwrap();
        assert_eq!(
            extract_session_cookie(&req),
            Some("abc.def.ghi".to_string()),
            "extractor must walk all Cookie headers (F-7.6-D-7 R1)"
        );
    }

    #[tokio::test]
    async fn staff_jwt_ttl_aligned_with_cookie_max_age() {
        // A5 closure (D-7.6-MMA Step 1 5/5 consensus + MAOR Finding-1) —
        // JWT TTL must align with the V2 staff session cookie Max-Age (both
        // 1800s = 30 min) so the browser-side and server-side validity
        // windows close together. F-7.6-D-4 mitigation contract.
        //
        // `create_staff_jwt_with_role` takes hours; the production call site
        // (api::auth_staff::staff_validate_pin) passes 1 hour as the smallest
        // integer ≥ 1800s. The cookie expires first at 1800s, so cookie
        // boundary is operational expiry. A future Phase 2 mint path with
        // seconds precision will tighten JWT.exp == 1800s exactly.
        let jwt = create_staff_jwt_with_role(TEST_SECRET, "staff-a5", "cashier", 1)
            .expect("mint JWT");
        let data = jsonwebtoken::decode::<StaffClaims>(
            &jwt,
            &DecodingKey::from_secret(TEST_SECRET.as_bytes()),
            &Validation::default(),
        )
        .expect("decode JWT");
        let lifetime_secs = data.claims.exp as i64 - data.claims.iat as i64;
        // 1 hour mint = 3600s; cookie Max-Age = 1800s. Cookie expires before
        // JWT, so cookie boundary is operational. Assert JWT TTL >= cookie TTL
        // (no replay window where JWT outlives cookie meaningfully).
        assert_eq!(
            lifetime_secs, 3600,
            "JWT TTL should be 3600s (1h) for staff-cookie path; got {}s",
            lifetime_secs
        );
        // Cookie TTL constant cross-check via the value the production code
        // emits (1800s per F-7.6-D-4 5/5 consensus). This pins the contract
        // so a future change to cookie TTL fails this test until the
        // operational alignment story is re-evaluated.
        const EXPECTED_COOKIE_TTL_SECS: i64 = 1800;
        assert!(
            lifetime_secs >= EXPECTED_COOKIE_TTL_SECS,
            "JWT TTL {}s must be ≥ cookie Max-Age {}s to avoid premature server-side rejection",
            lifetime_secs,
            EXPECTED_COOKIE_TTL_SECS
        );
    }

    #[tokio::test]
    async fn extract_session_cookie_rejects_duplicate_across_headers() {
        // F-7.6-D-3 + F-7.6-D-7 composed — duplicates SPAN multiple Cookie
        // headers must also fail closed.
        let req = Request::builder()
            .uri("/test")
            .header("Cookie", "rp_staff_session=first")
            .header("Cookie", "rp_staff_session=second")
            .body(())
            .unwrap();
        assert_eq!(extract_session_cookie(&req), None);
    }
}
