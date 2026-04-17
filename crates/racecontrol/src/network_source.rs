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
/// Pod IPs: 192.168.31.{28,33,38,86,87,88,89,91,130} + POS Tailscale 100.95.211.1
/// (POS added per Phase 413 — LAN IP .130 for normal operation, Tailscale IP
/// 100.95.211.1 for LAN-outage fallback; authoritative source: CLAUDE.md Network Map.)
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
            // POS Tailscale IP — narrow single-IP exception so POS rc-agent can
            // fetch the mesh key during LAN outages. DO NOT widen to 100.x.x.x —
            // that range also contains Bono VPS (Cloud) and server (Staff via LAN).
            // See CLAUDE.md Network Map for the authoritative POS Tailscale IP.
            if octets == [100, 95, 211, 1] {
                return RequestSource::Pod;
            }
            if octets[0] == 192 && octets[1] == 168 && octets[2] == 31 {
                match octets[3] {
                    28 | 33 | 38 | 86 | 87 | 88 | 89 | 91 | 130 => RequestSource::Pod,
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

/// Guard middleware: rejects requests from non-Pod sources with 403 Forbidden.
/// Fail-closed: if RequestSource extension is missing, rejects (unlike
/// require_non_pod_source which fails open).
/// Must run AFTER `classify_source_middleware` has inserted `RequestSource`.
/// Used by `/api/v1/pods/mesh-service-key` — only pods may fetch the mesh key.
pub async fn require_pod_source(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let source = req.extensions().get::<RequestSource>().copied();
    if source != Some(RequestSource::Pod) {
        return (
            axum::http::StatusCode::FORBIDDEN,
            "Pod source required",
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
        // Phase 413: 130 (POS LAN) added to Pod classification
        let pod_octets = [28, 33, 38, 86, 87, 88, 89, 91, 130];
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
    fn pos_ip_130_classifies_as_pod() {
        let ip: std::net::IpAddr = "192.168.31.130".parse().unwrap();
        assert_eq!(classify_ip(ip), RequestSource::Pod, "POS LAN .130 must be Pod per Phase 413");
    }

    #[test]
    fn pos_tailscale_classifies_as_pod() {
        // B1 fix — POS falls through to Tailscale (100.95.211.1) when LAN is down.
        // Without this, POS rc-agent would 403 on /pods/mesh-service-key during outages.
        let ip: std::net::IpAddr = "100.95.211.1".parse().unwrap();
        assert_eq!(classify_ip(ip), RequestSource::Pod, "POS Tailscale 100.95.211.1 must be Pod");
    }

    #[test]
    fn bono_vps_tailscale_stays_cloud() {
        // Regression guard: the narrow POS-Tailscale exception must NOT widen to 100.x.x.x.
        // Bono VPS on Tailscale is 100.70.177.44 — must remain Cloud.
        let ip: std::net::IpAddr = "100.70.177.44".parse().unwrap();
        assert_eq!(classify_ip(ip), RequestSource::Cloud, "Bono VPS Tailscale must stay Cloud");
    }

    #[test]
    fn server_tailscale_stays_cloud() {
        // Regression guard: server Tailscale (100.125.108.37) is "Cloud" class
        // (staff reach server via LAN .23; Tailscale is for external admin).
        let ip: std::net::IpAddr = "100.125.108.37".parse().unwrap();
        assert_eq!(classify_ip(ip), RequestSource::Cloud, "Server Tailscale must stay Cloud");
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

    // ── require_pod_source integration tests (Phase 413) ────────────────

    fn test_router_with_pod_guard() -> Router {
        Router::new()
            .route("/pod-only", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(require_pod_source))
    }

    #[tokio::test]
    async fn pod_guard_allows_pod_source() {
        let app = test_router_with_pod_guard();
        let req = axum::http::Request::builder()
            .uri("/pod-only")
            .extension(RequestSource::Pod)
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn pod_guard_rejects_staff_source() {
        let app = test_router_with_pod_guard();
        let req = axum::http::Request::builder()
            .uri("/pod-only")
            .extension(RequestSource::Staff)
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn pod_guard_rejects_customer_source() {
        let app = test_router_with_pod_guard();
        let req = axum::http::Request::builder()
            .uri("/pod-only")
            .extension(RequestSource::Customer)
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn pod_guard_rejects_cloud_source() {
        let app = test_router_with_pod_guard();
        let req = axum::http::Request::builder()
            .uri("/pod-only")
            .extension(RequestSource::Cloud)
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn pod_guard_rejects_missing_source() {
        // Fail-closed: unlike require_non_pod_source, this REJECTS when extension is missing
        let app = test_router_with_pod_guard();
        let req = axum::http::Request::builder()
            .uri("/pod-only")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
    }
}
