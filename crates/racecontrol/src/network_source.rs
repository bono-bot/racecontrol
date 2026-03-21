//! Network source classification for request origin tagging.
//!
//! Tags every incoming request as Pod, Staff, Customer, or Cloud based on
//! the client IP address. Used to restrict pod-originated requests from
//! accessing staff/admin routes.

use axum::extract::ConnectInfo;
use axum::response::IntoResponse;
use std::net::SocketAddr;

/// Classifies the origin of an HTTP request by source IP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestSource {
    /// Known pod IPs -- agent-level trust (kiosk routes only)
    Pod,
    /// Server, James workstation, POS PC -- admin trust
    Staff,
    /// Other LAN IPs on 192.168.31.* -- customer WiFi
    Customer,
    /// External / non-LAN IPs -- cloud sync trust
    Cloud,
}

/// Pure function: classifies an IP address into a RequestSource.
///
/// Pod IPs: 192.168.31.{28,33,38,86,87,88,89,91}
/// Staff IPs: 192.168.31.{20,23,27}, 127.0.0.1, ::1
/// Customer: other 192.168.31.* addresses
/// Cloud: everything else
pub fn classify_ip(ip: std::net::IpAddr) -> RequestSource {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let octets = v4.octets();
            if octets == [127, 0, 0, 1] {
                return RequestSource::Staff;
            }
            if octets[0] == 192 && octets[1] == 168 && octets[2] == 31 {
                match octets[3] {
                    28 | 33 | 38 | 86 | 87 | 88 | 89 | 91 => RequestSource::Pod,
                    20 | 23 | 27 => RequestSource::Staff,
                    _ => RequestSource::Customer,
                }
            } else {
                RequestSource::Cloud
            }
        }
        std::net::IpAddr::V6(v6) => {
            if v6.is_loopback() {
                RequestSource::Staff
            } else {
                RequestSource::Cloud
            }
        }
    }
}

/// Axum middleware: extracts client IP from ConnectInfo, classifies it,
/// and inserts `RequestSource` into request extensions for downstream use.
pub async fn classify_source_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let source = classify_ip(addr.ip());
    req.extensions_mut().insert(source);
    next.run(req).await
}

/// Guard middleware: rejects requests from Pod sources with 403 Forbidden.
/// Must run AFTER `classify_source_middleware` has inserted `RequestSource`.
pub async fn require_non_pod_source(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let source = req.extensions().get::<RequestSource>().copied();
    if source == Some(RequestSource::Pod) {
        return (
            axum::http::StatusCode::FORBIDDEN,
            "Pod source not allowed on staff routes",
        )
            .into_response();
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::Router;
    use std::net::SocketAddr;
    use tower::ServiceExt;

    // ── classify_ip unit tests ──────────────────────────────────────────

    #[test]
    fn pod_ips_classify_as_pod() {
        let pod_octets = [28, 33, 38, 86, 87, 88, 89, 91];
        for last in pod_octets {
            let ip: std::net::IpAddr = format!("192.168.31.{}", last).parse().unwrap();
            assert_eq!(
                classify_ip(ip),
                RequestSource::Pod,
                "192.168.31.{} should be Pod",
                last
            );
        }
    }

    #[test]
    fn staff_ips_classify_as_staff() {
        let staff_octets = [20, 23, 27];
        for last in staff_octets {
            let ip: std::net::IpAddr = format!("192.168.31.{}", last).parse().unwrap();
            assert_eq!(
                classify_ip(ip),
                RequestSource::Staff,
                "192.168.31.{} should be Staff",
                last
            );
        }
    }

    #[test]
    fn localhost_classifies_as_staff() {
        let ip: std::net::IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(classify_ip(ip), RequestSource::Staff);
    }

    #[test]
    fn ipv6_loopback_classifies_as_staff() {
        let ip: std::net::IpAddr = "::1".parse().unwrap();
        assert_eq!(classify_ip(ip), RequestSource::Staff);
    }

    #[test]
    fn customer_wifi_classifies_as_customer() {
        let ip: std::net::IpAddr = "192.168.31.100".parse().unwrap();
        assert_eq!(classify_ip(ip), RequestSource::Customer);
    }

    #[test]
    fn external_ip_classifies_as_cloud() {
        let ip: std::net::IpAddr = "72.60.101.58".parse().unwrap();
        assert_eq!(classify_ip(ip), RequestSource::Cloud);
    }

    #[test]
    fn other_private_range_classifies_as_cloud() {
        let ip: std::net::IpAddr = "10.0.0.1".parse().unwrap();
        assert_eq!(classify_ip(ip), RequestSource::Cloud);
    }

    // ── require_non_pod_source integration tests ────────────────────────

    fn test_router_with_guard() -> Router {
        Router::new()
            .route("/protected", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(require_non_pod_source))
    }

    #[tokio::test]
    async fn guard_rejects_pod_source_with_403() {
        let app = test_router_with_guard();
        let req = axum::http::Request::builder()
            .uri("/protected")
            .extension(RequestSource::Pod)
            .body(axum::body::Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn guard_allows_staff_source() {
        let app = test_router_with_guard();
        let req = axum::http::Request::builder()
            .uri("/protected")
            .extension(RequestSource::Staff)
            .body(axum::body::Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn guard_allows_customer_source() {
        let app = test_router_with_guard();
        let req = axum::http::Request::builder()
            .uri("/protected")
            .extension(RequestSource::Customer)
            .body(axum::body::Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn guard_allows_cloud_source() {
        let app = test_router_with_guard();
        let req = axum::http::Request::builder()
            .uri("/protected")
            .extension(RequestSource::Cloud)
            .body(axum::body::Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn guard_allows_missing_source() {
        // If classify_source_middleware didn't run, no extension present -- allow through
        let app = test_router_with_guard();
        let req = axum::http::Request::builder()
            .uri("/protected")
            .body(axum::body::Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }
}
