use axum::Router;
use axum::routing::get;
use axum::middleware as axum_mw;
use axum::extract::State;
use axum::response::IntoResponse;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{CorsLayer, AllowOrigin};
use axum::http::{HeaderValue, Method, StatusCode};
use tower_http::trace::TraceLayer;

use racecontrol_crate::config::Config;
use racecontrol_crate::state::AppState;
use racecontrol_crate::{
    ac_camera, ac_server, accounting, action_queue, activity_log, ai, api, auth,
    billing, bono_relay, catalog, cloud_sync, config, db, error_aggregator, fleet_health, friends,
    game_launcher, multiplayer, port_allocator, lap_tracker, pod_healer,
    pod_monitor, pod_reservation, remote_terminal, scheduler, server_ops, wallet,
    udp_heartbeat, wol, ws,
};

/// Sends a test email on first boot to verify Gmail OAuth works.
/// Uses a flag file (`./data/email_verified.flag`) to prevent repeat sends.
/// The flag is written regardless of send success to prevent spam on misconfiguration.
async fn maybe_send_first_boot_email(state: &std::sync::Arc<AppState>) {
    const FLAG_PATH: &str = "./data/email_verified.flag";

    // Check if we've already run the first-boot email check
    if std::path::Path::new(FLAG_PATH).exists() {
        return;
    }

    // Ensure the data directory exists
    if let Err(e) = std::fs::create_dir_all("./data") {
        tracing::warn!("Could not create ./data directory for email flag: {}", e);
    }

    // Write the flag file first (prevents spam even if send fails)
    if let Err(e) = std::fs::write(FLAG_PATH, "1") {
        tracing::warn!("Could not write email_verified.flag: {}", e);
    }

    // Check if email alerts are enabled by checking should_send (disabled alerter always returns false)
    {
        let alerter = state.email_alerter.read().await;
        if !alerter.should_send("system", chrono::Utc::now()) {
            tracing::info!("First-boot email check: email alerts disabled or rate-limited, skipping.");
            return;
        }
    }

    // Attempt to send the test email
    tracing::info!("First-boot: sending test email to verify Gmail OAuth...");
    let mut alerter = state.email_alerter.write().await;
    alerter
        .send_alert(
            "system",
            "RaceControl Started - Email Alerts Active",
            "RaceControl has started successfully. Email alerts are configured and working.",
        )
        .await;
    tracing::info!("First-boot email send attempted. Check logs for delivery status.");
}

/// Middleware: if a JSON response body contains "JWT decode error", set status to 401
async fn jwt_error_to_401(
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
        if let Ok(s) = std::str::from_utf8(&body_bytes) {
            if s.contains("JWT decode error") || s.contains("Missing Authorization") {
                parts.status = axum::http::StatusCode::UNAUTHORIZED;
            }
        }
        return axum::response::Response::from_parts(parts, axum::body::Body::from(body_bytes));
    }

    axum::response::Response::from_parts(parts, body)
}

/// Web dashboard paths that get proxied to the staff dashboard on port 3200.
const WEB_DASHBOARD_PATHS: &[&str] = &[
    "/billing", "/presenter", "/leaderboards", "/drivers", "/pods",
    "/telemetry", "/games", "/ai", "/sessions", "/bookings",
    "/events", "/settings", "/ac-lan", "/ac-sessions", "/results",
];

/// Reverse proxy: forwards /kiosk* and /_next/* to the local Next.js kiosk on port 3300,
/// and web dashboard paths to the staff dashboard on port 3200.
/// This bypasses Windows Smart App Control which blocks node.exe from accepting network connections.
async fn kiosk_proxy(
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
    let is_kiosk = path_and_query.starts_with("/kiosk");
    let is_dashboard = path_and_query.starts_with("/_next")
        || WEB_DASHBOARD_PATHS.iter().any(|p| path_and_query.starts_with(p));

    if !is_kiosk && !is_dashboard {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }

    let port = if is_kiosk { 3300 } else { 3200 };
    let url = format!("http://127.0.0.1:{}{}", port, path_and_query);
    let method = req.method().clone();

    // Forward select headers (skip host, connection, etc.)
    let mut proxy_headers = reqwest::header::HeaderMap::new();
    for (key, val) in req.headers() {
        let name = key.as_str();
        if name != "host" && name != "connection" {
            if let Ok(k) = reqwest::header::HeaderName::from_bytes(key.as_ref()) {
                if let Ok(v) = reqwest::header::HeaderValue::from_bytes(val.as_bytes()) {
                    proxy_headers.insert(k, v);
                }
            }
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
                return builder.body(axum::body::Body::from(body)).unwrap().into_response();
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
    axum::response::Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .header("content-type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(backend_unavailable_page(service, port)))
        .unwrap()
        .into_response()
}

/// Branded error page shown when the kiosk or dashboard backend is unreachable.
/// Auto-reloads every 5 seconds so it recovers automatically once the service starts.
fn backend_unavailable_page(service: &str, port: u16) -> String {
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing — dual output: stdout + rolling log file
    let log_dir = std::path::Path::new("logs");
    std::fs::create_dir_all(log_dir).ok();
    let file_appender = tracing_appender::rolling::daily(log_dir, "racecontrol.log");
    let (non_blocking_file, _guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "racecontrol_crate=info,tower_http=info".into());

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_ansi(false)
                .with_writer(non_blocking_file),
        )
        .init();

    println!(r#"
    ____                  ______            __             __
   / __ \____ _________  / ____/___  ____  / /__________  / /
  / /_/ / __ `/ ___/ _ \/ /   / __ \/ __ \/ __/ ___/ __ \/ /
 / _, _/ /_/ / /__/  __/ /___/ /_/ / / / / /_/ /  / /_/ / /
/_/ |_|\__,_/\___/\___/\____/\____/_/ /_/\__/_/   \____/_/

  Sim Racing Venue Management System
  by RacingPoint
"#);

    // Load config
    let config = Config::load_or_default();

    // Warn if default JWT secret is unchanged
    if config.auth.jwt_secret == "racingpoint-jwt-change-me-in-production" {
        tracing::warn!("Using default JWT secret! Set auth.jwt_secret in racecontrol.toml for production.");
    }
    tracing::info!("Venue: {} ({})", config.venue.name, config.venue.location);
    tracing::info!("Server: {}:{}", config.server.host, config.server.port);

    // Initialize database
    let pool = db::init_pool(&config.database.path).await?;

    // Build application state
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let state = Arc::new(AppState::new(config, pool));

    // First-boot email test: verify Gmail OAuth works on initial setup
    maybe_send_first_boot_email(&state).await;

    // Clean up orphaned acServer processes from previous run
    // (must happen after AppState is built so port_allocator can track freed ports)
    match ac_server::cleanup_orphaned_sessions(&state.db, &state.port_allocator).await {
        Ok(0) => tracing::info!("No orphaned AC sessions found"),
        Ok(n) => tracing::warn!("Cleaned up {} orphaned AC sessions on startup", n),
        Err(e) => tracing::error!("Failed to clean up orphaned sessions: {}", e),
    }

    // Recover any active billing sessions from DB
    billing::recover_active_sessions(&state).await?;

    // Load billing rate tiers from DB into cache
    billing::refresh_rate_tiers(&state).await;

    // Spawn billing tick loop (1 second interval, refresh rates every 60s)
    let tick_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut refresh_counter: u32 = 0;
        loop {
            interval.tick().await;
            billing::tick_all_timers(&tick_state).await;
            refresh_counter += 1;
            if refresh_counter >= 60 {
                refresh_counter = 0;
                billing::refresh_rate_tiers(&tick_state).await;
            }
        }
    });

    // Spawn billing DB sync loop (5 second interval)
    let sync_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            billing::sync_timers_to_db(&sync_state).await;
        }
    });

    // Spawn game health check loop (5 second interval)
    let game_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            game_launcher::check_game_health(&game_state).await;
        }
    });

    // Spawn AC server health check loop (5 second interval)
    let ac_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            ac_server::check_ac_server_health(&ac_state).await;
        }
    });

    // Spawn auth token expiry loop (30 second interval)
    let auth_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            auth::expire_stale_tokens(&auth_state).await;
        }
    });

    // Spawn pod reservation expiry loop (30 second interval)
    let res_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            pod_reservation::expire_idle_reservations(&res_state).await;
        }
    });

    // Spawn camera control tick loop (2 second interval)
    let cam_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            ac_camera::tick(&cam_state).await;
        }
    });

    // Spawn proactive error pattern detection
    error_aggregator::spawn(state.clone());

    // Spawn cloud sync (pulls customer data from cloud racecontrol)
    cloud_sync::spawn(state.clone());

    // Spawn Bono relay (pushes events to Bono's VPS over Tailscale mesh)
    bono_relay::spawn(state.clone());

    // Spawn remote terminal (polls cloud for commands to execute locally)
    remote_terminal::spawn(state.clone());

    // Spawn action queue (polls cloud for pending actions — bookings, wallet, QR, etc.)
    action_queue::spawn(state.clone());

    // Spawn pod monitor (Tier 2: detect stale pods, auto-restart via pod-agent)
    pod_monitor::spawn(state.clone());

    // Spawn pod healer (Tier 3: deep diagnostics, auto-fix zombies, AI escalation)
    pod_healer::spawn(state.clone());

    // Spawn smart scheduler (auto-wake/shutdown pods, peak hour tracking)
    scheduler::spawn(state.clone());

    // Spawn UDP heartbeat listener (fast liveness detection alongside WebSocket)
    udp_heartbeat::spawn(state.clone());

    // Spawn fleet health probe loop (15s interval, HTTP :8090/health on each registered pod)
    fleet_health::start_probe_loop(state.clone());

    // Start server_ops HTTP endpoint on :8090 (remote command execution, file ops)
    server_ops::start();

    // Bind Bono relay endpoint on Tailscale IP (optional — only if configured)
    // IMPORTANT: state.clone() is called here before state is moved into the main router below.
    if let Some(ts_ip) = state.config.bono.tailscale_bind_ip.clone() {
        if state.config.bono.enabled {
            let relay_port = state.config.bono.relay_port;
            let ts_addr = format!("{}:{}", ts_ip, relay_port);
            let relay_router = bono_relay::build_relay_router(state.clone());
            tokio::spawn(async move {
                match tokio::net::TcpListener::bind(&ts_addr).await {
                    Ok(ts_listener) => {
                        tracing::info!("Bono relay endpoint on http://{} (Tailscale interface)", ts_addr);
                        axum::serve(ts_listener, relay_router).await.unwrap();
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to bind Bono relay endpoint on {} — check Tailscale is connected: {}",
                            ts_addr, e
                        );
                        // Non-fatal: main server continues even if relay bind fails.
                        // This happens when Tailscale isn't yet connected at startup.
                    }
                }
            });
        }
    }

    // Build router
    let app = Router::new()
        // API routes
        .nest("/api/v1", api::routes::api_routes())
        // WebSocket endpoints
        .route("/ws/agent", get(ws::agent_ws))
        .route("/ws/dashboard", get(ws::dashboard_ws))
        .route("/ws/ai", get(ws::ai_ws))
        // Registration page (standalone HTML for QR code walk-in flow)
        .route("/register", get(|| async {
            axum::response::Html(include_str!("../../../assets/register.html"))
        }))
        // Health check at root
        .route("/", get(|| async {
            axum::Json(serde_json::json!({
                "name": "RaceControl",
                "status": "running",
                "version": env!("CARGO_PKG_VERSION"),
            }))
        }))
        // Reverse proxy: kiosk UI + Next.js assets → localhost:3300
        .fallback(kiosk_proxy)
        .layer(axum_mw::from_fn(jwt_error_to_401))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
                    let origin = origin.to_str().unwrap_or("");
                    origin.starts_with("http://localhost:")
                        || origin.starts_with("http://127.0.0.1:")
                        || origin.starts_with("http://192.168.31.")
                        || origin.starts_with("http://kiosk.rp")
                        || origin.contains("racingpoint.cloud")
                }))
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE, Method::OPTIONS])
                .allow_headers(tower_http::cors::Any)
                .allow_credentials(false)
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("RaceControl listening on http://{}", bind_addr);
    tracing::info!("Dashboard:   http://{}/", bind_addr);
    tracing::info!("API:         http://{}/api/v1/health", bind_addr);
    tracing::info!("Agent WS:    ws://{}/ws/agent", bind_addr);
    tracing::info!("Dashboard WS: ws://{}/ws/dashboard", bind_addr);
    tracing::info!("AI WS:        ws://{}/ws/ai", bind_addr);

    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_unavailable_page_is_html() {
        let html = backend_unavailable_page("Kiosk", 3300);
        assert!(html.contains("<!DOCTYPE html>"), "must be a full HTML page");
    }

    #[test]
    fn backend_unavailable_page_has_branding() {
        let html = backend_unavailable_page("Dashboard", 3200);
        assert!(html.contains("#E10600"), "must contain Racing Point red");
        assert!(html.contains("RACING"), "must contain RACING wordmark");
        assert!(html.contains("POINT"), "must contain POINT wordmark");
    }

    #[test]
    fn backend_unavailable_page_has_auto_retry() {
        let html = backend_unavailable_page("Kiosk", 3300);
        assert!(html.contains("location.reload"), "must auto-reload");
    }

    #[test]
    fn backend_unavailable_page_shows_service_name() {
        let kiosk = backend_unavailable_page("Kiosk", 3300);
        assert!(kiosk.contains("Kiosk STARTING UP"), "must show Kiosk service name");
        assert!(kiosk.contains("3300"), "must show kiosk port");

        let dash = backend_unavailable_page("Dashboard", 3200);
        assert!(dash.contains("Dashboard STARTING UP"), "must show Dashboard service name");
        assert!(dash.contains("3200"), "must show dashboard port");
    }
}
