//! HTTP middleware and proxy handlers for the RaceControl server.
//!
//! Contains: JWT error rewriting, kiosk/dashboard reverse proxy,
//! security headers (CSP, HSTS), cache control, and the branded error page.
//!
//! Extracted from main.rs to keep the binary entrypoint under 500 lines.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware as axum_mw;
use axum::response::IntoResponse;
use tower_helmet::HelmetLayer;

use racecontrol_crate::state::AppState;

/// Middleware: if a JSON response body contains "JWT decode error", set status to 401
pub async fn jwt_error_to_401(
    req: axum::extract::Request,
    next: axum_mw::Next,
) -> axum::response::Response {
    let res = next.run(req).await;
    let (mut parts, body) = res.into_parts();

    // Only check 200 JSON responses
    let is_json = parts.headers.get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("json"))
        .unwrap_or(false);

    if parts.status == axum::http::StatusCode::OK && is_json {
        let body_bytes = axum::body::to_bytes(body, 1024 * 64).await.unwrap_or_default();
        if let Ok(s) = std::str::from_utf8(&body_bytes)
            && (s.contains("JWT decode error") || s.contains("Missing Authorization")) {
                parts.status = axum::http::StatusCode::UNAUTHORIZED;
            }
        return axum::response::Response::from_parts(parts, axum::body::Body::from(body_bytes));
    }

    axum::response::Response::from_parts(parts, body)
}

/// Web dashboard paths that get proxied to the staff dashboard on port 3200.
/// MMA Round 4 P1 fix: /login was missing — AuthGate redirects unauthenticated
/// users to /login, causing 404 on POS billing kiosk. Added ALL web app routes.
const WEB_DASHBOARD_PATHS: &[&str] = &[
    "/billing", "/presenter", "/leaderboards", "/drivers", "/pods",
    "/telemetry", "/games", "/ai", "/sessions", "/bookings",
    "/events", "/settings", "/ac-lan", "/ac-sessions", "/results",
    "/login", "/cameras", "/cafe", "/book", "/flags", "/ota",
];

/// Reverse proxy: forwards /kiosk* and /_next/* to the local Next.js kiosk on port 3300,
/// and web dashboard paths to the staff dashboard on port 3200.
/// This bypasses Windows Smart App Control which blocks node.exe from accepting network connections.
pub async fn kiosk_proxy(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    let path_and_query = req.uri().path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    // Route to the correct backend based on path:
    // - /kiosk* (including /kiosk/_next/*) → kiosk on port 3300
    // - /_next/* (bare, no /kiosk prefix) → web dashboard on port 3200
    // - Dashboard pages (/billing, /presenter, etc.) → web dashboard on port 3200
    //
    // MMA Round 1 P2 fix (2/3 consensus): Handle /kiosk/<dashboard-path> gracefully.
    // The kiosk app has NO /billing page — if someone navigates to /kiosk/billing,
    // redirect them to /billing (the web dashboard) instead of returning a 404.
    let is_kiosk = path_and_query.starts_with("/kiosk");
    let is_dashboard = path_and_query.starts_with("/_next")
        || WEB_DASHBOARD_PATHS.iter().any(|p| path_and_query.starts_with(p));

    // MMA Round 2 fixes (3-model consensus):
    // - P1: Use strip_prefix (safe, no panic on edge cases)
    // - P1: Validate redirect target starts with "/" (prevent open redirect via //evil.com)
    // - P2: Use temporary redirect (307, not 308) — avoid permanent cache in kiosk Edge
    // - P2: Exact path segment match (not starts_with) — prevent /kiosk/billing-old matching
    if is_kiosk
        && let Some(after_kiosk) = path_and_query.strip_prefix("/kiosk") {
            // Extract just the path portion (before any query string) for matching
            let path_part = after_kiosk.split('?').next().unwrap_or(after_kiosk);
            // Exact match: path must be exactly a dashboard path OR dashboard path + "/"
            let is_dashboard_redirect = WEB_DASHBOARD_PATHS.iter().any(|p| {
                path_part == *p || path_part.starts_with(&format!("{}/", p))
            });
            // Security: redirect target must start with "/" (not "//") to prevent open redirect
            if is_dashboard_redirect && after_kiosk.starts_with('/') && !after_kiosk.starts_with("//") {
                return axum::response::Redirect::temporary(after_kiosk).into_response();
            }
        }

    // PERMANENT FIX (Unified Protocol): For paths not explicitly in WEB_DASHBOARD_PATHS
    // and not /kiosk*, try the web dashboard (port 3200) first. If it returns 404,
    // then return 404. This prevents new pages added to the web app from being blocked
    // by a stale static path list. The WEB_DASHBOARD_PATHS list is kept for the
    // /kiosk/* redirect logic but is no longer the gatekeeper for proxy routing.
    let is_unknown = !is_kiosk && !is_dashboard;
    let try_web_for_unknown = is_unknown
        && path_and_query.starts_with('/')
        && !path_and_query.starts_with("//")
        && !path_and_query.contains("..");

    if is_unknown && !try_web_for_unknown {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }

    let port = if is_kiosk { 3300 } else { 3200 };
    let url = format!("http://127.0.0.1:{}{}", port, path_and_query);
    let method = req.method().clone();

    // Forward select headers (skip host, connection, etc.)
    let mut proxy_headers = reqwest::header::HeaderMap::new();
    for (key, val) in req.headers() {
        let name = key.as_str();
        if name != "host" && name != "connection"
            && let Ok(k) = reqwest::header::HeaderName::from_bytes(key.as_ref())
                && let Ok(v) = reqwest::header::HeaderValue::from_bytes(val.as_bytes()) {
                    proxy_headers.insert(k, v);
                }
    }

    let body_bytes = match axum::body::to_bytes(req.into_body(), 10_000_000).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST, "Body too large").into_response(),
    };

    // Retry up to 3 times with 1s backoff to absorb backend startup delay.
    // This eliminates the 502 error page during the typical 3-5s Node.js boot window.
    let mut last_err = String::new();
    for attempt in 0..3u8 {
        let req_method = method.clone();
        let req_headers = proxy_headers.clone();
        let req_body = body_bytes.clone();

        let resp = state.http_client
            .request(req_method, &url)
            .headers(req_headers)
            .body(req_body)
            .timeout(Duration::from_secs(10))
            .send()
            .await;

        match resp {
            Ok(r) => {
                let status = StatusCode::from_u16(r.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                let mut builder = axum::response::Response::builder().status(status);
                for (key, val) in r.headers() {
                    let k = key.as_str();
                    if k == "transfer-encoding" || k == "connection" || k == "keep-alive" {
                        continue;
                    }
                    builder = builder.header(k, val.as_bytes());
                }
                let body = r.bytes().await.unwrap_or_default();
                if attempt > 0 {
                    tracing::info!("Proxy succeeded on attempt {} for {}", attempt + 1, url);
                }
                return match builder.body(axum::body::Body::from(body)) {
                    Ok(resp) => resp.into_response(),
                    Err(e) => {
                        tracing::error!("Failed to build proxy response: {e}");
                        StatusCode::INTERNAL_SERVER_ERROR.into_response()
                    }
                };
            }
            Err(e) => {
                last_err = format!("{e}");
                if attempt < 2 {
                    tracing::info!("Proxy attempt {} failed for {}, retrying in 1s: {e}", attempt + 1, url);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    // All 3 retries exhausted — show branded error page
    tracing::warn!("Proxy failed after 3 attempts for {}: {}", url, last_err);
    let service = if is_kiosk { "Kiosk" } else { "Dashboard" };
    match axum::response::Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .header("content-type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(backend_unavailable_page(service, port)))
    {
        Ok(resp) => resp.into_response(),
        Err(e) => {
            tracing::error!("Failed to build error page response: {e}");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

/// Branded error page shown when the kiosk or dashboard backend is unreachable.
/// Auto-reloads every 5 seconds so it recovers automatically once the service starts.
pub fn backend_unavailable_page(service: &str, port: u16) -> String {
    format!(r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Racing Point — {service} Unavailable</title>
<link href="https://fonts.googleapis.com/css2?family=Montserrat:wght@300;400;600;700;800&display=swap" rel="stylesheet">
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{
    background: linear-gradient(135deg, #1A1A1A 0%, #222222 50%, #1A1A1A 100%);
    color: #fff;
    font-family: 'Montserrat', 'Segoe UI', system-ui, sans-serif;
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
}}
@keyframes spin {{
    0%   {{ transform: rotate(0deg); }}
    100% {{ transform: rotate(360deg); }}
}}
@keyframes pulse {{
    0%, 100% {{ opacity: 1; }}
    50% {{ opacity: 0.5; }}
}}
</style>
</head>
<body>
<div style="text-align:center">
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 220 64" width="220" height="64" role="img" aria-label="Racing Point" style="margin-bottom:40px">
  <rect x="0" y="4" width="8" height="8" fill="#E10600"/>
  <rect x="8" y="4" width="8" height="8" fill="#ffffff" opacity="0.15"/>
  <rect x="0" y="12" width="8" height="8" fill="#ffffff" opacity="0.15"/>
  <rect x="8" y="12" width="8" height="8" fill="#E10600"/>
  <text x="24" y="36" font-family="Montserrat,Segoe UI,system-ui,sans-serif" font-weight="800" font-size="26" letter-spacing="4" fill="#E10600">RACING</text>
  <text x="24" y="58" font-family="Montserrat,Segoe UI,system-ui,sans-serif" font-weight="300" font-size="18" letter-spacing="6" fill="#ffffff" opacity="0.85">POINT</text>
</svg>
<div style="font-size:1.6em;font-weight:700;color:#E10600;margin-bottom:16px;letter-spacing:2px">{service} STARTING UP</div>
<div style="font-size:1em;color:#888;margin-bottom:8px">The {service} service on port {port} is not ready yet.</div>
<div style="font-size:0.9em;color:#5A5A5A;margin-bottom:40px">This page will automatically retry.</div>
<div style="display:inline-block;width:48px;height:48px;border:4px solid #333;border-top-color:#E10600;border-radius:50%;animation:spin 0.9s linear infinite"></div>
<div style="margin-top:40px;font-size:0.75em;color:#333;animation:pulse 2s infinite">Retrying in <span id="cd">5</span>s</div>
</div>
<script>
var s=5,el=document.getElementById('cd');
setInterval(function(){{ s--; if(s<=0){{ location.reload(); }} else {{ el.textContent=s; }} }},1000);
</script>
</body>
</html>"##, service = service, port = port)
}

/// Build security headers middleware via tower-helmet.
///
/// Sets CSP, X-Frame-Options (DENY), X-Content-Type-Options (nosniff),
/// and HSTS with a short max-age (300s / 5 minutes) for initial deploy safety.
/// The short HSTS avoids browser lockout during testing — increase after 1 week stable.
pub fn security_headers_layer() -> HelmetLayer {
    use tower_helmet::header::{
        ContentSecurityPolicy, StrictTransportSecurity, XFrameOptions,
    };

    let mut directives = std::collections::HashMap::new();
    directives.insert("default-src", vec!["'self'"]);
    // 'unsafe-inline' needed for portal.html, status.html, register.html which use
    // inline <script> tags for health checks and clock. Without it, CSP blocks the
    // scripts and portal shows permanent "Connecting" (orange) status.
    directives.insert("script-src", vec!["'self'", "'unsafe-inline'"]);
    directives.insert("style-src", vec!["'self'", "'unsafe-inline'"]);
    directives.insert("img-src", vec!["'self'", "data:"]);
    // Allow connect to self + venue server on any port (kiosk on :80 needs API on :8080).
    // CSP doesn't support IP wildcards, so list the server explicitly.
    // ws:/wss: scheme-only = any WS host (needed for dashboard WS to server).
    directives.insert("connect-src", vec![
        "'self'",
        "http://192.168.31.23:8080",   // server API from kiosk on :80/:3300
        "ws://192.168.31.23:8080",      // server WS from kiosk on :80/:3300
        "http://localhost:8080",        // dev
        "ws://localhost:8080",          // dev WS
        "ws:", "wss:",
    ]);
    directives.insert("frame-ancestors", vec!["'none'"]);
    directives.insert("base-uri", vec!["'self'"]);
    directives.insert("form-action", vec!["'self'"]);

    let csp = ContentSecurityPolicy {
        use_defaults: false,
        directives,
        report_only: false,
    };

    let hsts = StrictTransportSecurity {
        max_age: Duration::from_secs(300), // 5 min — safe for testing
        include_subdomains: true,
        preload: false,
    };

    let mut layer = HelmetLayer::blank();
    layer
        .enable(csp)
        .enable(XFrameOptions::Deny)
        .enable(tower_helmet::header::XContentTypeOptions)
        .enable(hsts);
    layer
}

/// Cache control middleware — matches cloud nginx strategy.
/// HTML/API: no-cache (always revalidate on update rollouts).
/// _next/static: immutable (content-hashed filenames, safe to cache forever).
/// Static assets (images): moderate cache with revalidation.
pub async fn cache_control_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = req.uri().path().to_owned();
    let mut response = next.run(req).await;

    // Don't override if backend (Next.js proxy) already set Cache-Control
    if response.headers().contains_key("cache-control") {
        return response;
    }

    let value = if path.starts_with("/_next/static/") || path.starts_with("/kiosk/_next/static/") {
        // Content-hashed static bundles — cache forever
        "public, max-age=31536000, immutable"
    } else if path.starts_with("/static/") {
        // Cafe images etc — cache 1hr, revalidate
        "public, max-age=3600, must-revalidate"
    } else if path.starts_with("/ws/") {
        // WebSocket upgrades — no caching
        return response;
    } else {
        // Everything else (HTML, API, portal, proxied pages): never cache
        "no-cache, no-store, must-revalidate"
    };

    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static(value),
    );
    response
}
