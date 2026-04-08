use std::sync::Arc;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::PeerIpKeyExtractor;
use tower_governor::GovernorLayer;

/// Rate limit layer for auth endpoints: 20 requests burst per IP, replenish 1/3s.
///
/// RESIL-03: Burst increased from 5 to 20 for venue LAN — at peak load, 8 pods + kiosk
/// can submit 9+ concurrent PIN validations. Previous burst=5 rejected 4/9 registrations
/// on shared LAN IP (NAT). Replenish rate increased from 12s to 3s per token.
///
/// Applied to: /auth/admin-login, /customer/login, /customer/verify-otp,
///             /auth/validate-pin, /auth/kiosk/validate-pin
///
/// Uses PeerIpKeyExtractor which reads ConnectInfo<SocketAddr> from request extensions.
/// Requires `into_make_service_with_connect_info::<SocketAddr>()` on the server.
pub fn auth_rate_limit_layer() -> GovernorLayer<PeerIpKeyExtractor, governor::middleware::NoOpMiddleware<governor::clock::QuantaInstant>, axum::body::Body> {
    let config = GovernorConfigBuilder::default()
        .per_second(3) // Replenish 1 token every 3 seconds (~20/min)
        .burst_size(20) // Max burst: 20 requests (handles 8 pods + kiosk + staff simultaneously)
        .finish()
        .expect("GovernorConfig builder: burst_size and period are non-zero");

    GovernorLayer::new(Arc::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::ConnectInfo;
    use axum::routing::get;
    use axum::Router;
    use std::net::SocketAddr;
    use tower::ServiceExt;

    fn test_router() -> Router {
        Router::new()
            .route(
                "/rate-limited",
                get(|| async { "ok" }),
            )
            .layer(auth_rate_limit_layer())
    }

    #[tokio::test]
    async fn rate_limit_first_twenty_requests_succeed() {
        let app = test_router();
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        for i in 0..20 {
            let req = axum::http::Request::builder()
                .uri("/rate-limited")
                .extension(ConnectInfo(addr))
                .body(axum::body::Body::empty())
                .unwrap();

            let resp: axum::http::Response<_> = app.clone().oneshot(req).await.unwrap();
            assert_eq!(
                resp.status(),
                axum::http::StatusCode::OK,
                "Request {} should succeed",
                i + 1
            );
        }
    }

    #[tokio::test]
    async fn rate_limit_21st_rapid_request_returns_429() {
        let app = test_router();
        let addr: SocketAddr = "127.0.0.1:12346".parse().unwrap();

        // Exhaust the 20-request burst
        for _ in 0..20 {
            let req = axum::http::Request::builder()
                .uri("/rate-limited")
                .extension(ConnectInfo(addr))
                .body(axum::body::Body::empty())
                .unwrap();
            let _: axum::http::Response<_> = app.clone().oneshot(req).await.unwrap();
        }

        // 21st request should be rate limited
        let req = axum::http::Request::builder()
            .uri("/rate-limited")
            .extension(ConnectInfo(addr))
            .body(axum::body::Body::empty())
            .unwrap();
        let resp: axum::http::Response<_> = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "21st rapid request should return 429"
        );
    }

    #[tokio::test]
    async fn rate_limit_different_ips_have_separate_limits() {
        let app = test_router();
        let addr1: SocketAddr = "10.0.0.1:1111".parse().unwrap();
        let addr2: SocketAddr = "10.0.0.2:2222".parse().unwrap();

        // Exhaust IP1's burst
        for _ in 0..20 {
            let req = axum::http::Request::builder()
                .uri("/rate-limited")
                .extension(ConnectInfo(addr1))
                .body(axum::body::Body::empty())
                .unwrap();
            let _: axum::http::Response<_> = app.clone().oneshot(req).await.unwrap();
        }

        // IP2 should still be allowed
        let req = axum::http::Request::builder()
            .uri("/rate-limited")
            .extension(ConnectInfo(addr2))
            .body(axum::body::Body::empty())
            .unwrap();
        let resp: axum::http::Response<_> = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            axum::http::StatusCode::OK,
            "Different IP should not be rate limited"
        );
    }
}
