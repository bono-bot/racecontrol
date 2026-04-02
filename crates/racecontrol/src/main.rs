use axum::Router;
use axum::routing::get;
use axum::middleware as axum_mw;
use axum::extract::State;
use axum::response::IntoResponse;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{CorsLayer, AllowOrigin};
use axum::http::{HeaderValue, Method, StatusCode};
use tower_http::trace::TraceLayer;
use tower_helmet::HelmetLayer;

use racecontrol_crate::config::Config;
use racecontrol_crate::crypto::encryption::load_encryption_keys;
use racecontrol_crate::db::migrate_pii_encryption;
use racecontrol_crate::network_source::classify_source_middleware;
use racecontrol_crate::tls;
use racecontrol_crate::error_rate::{ErrorCountLayer, ErrorRateConfig, error_rate_alerter_task};
use racecontrol_crate::state::AppState;
use rc_common::protocol::DashboardEvent;
use rc_common::types::{PodInfo, PodStatus, SimType};
use racecontrol_crate::{
    ac_camera, ac_server, action_queue, api, app_health_monitor, auth,
    backup_pipeline, billing, bono_relay, cloud_sync, db, deploy_awareness, error_aggregator,
    event_archive, fleet_health, game_launcher, pod_healer, pod_monitor, pod_reservation,
    process_guard, psychology, remote_terminal, scheduler, server_ops,
    udp_heartbeat, ws,
};

/// Auto-seed all 8 pods into the in-memory pods map on server startup.
/// Called immediately after AppState::new() so the kiosk is never left with
/// an empty pod list after a server restart with a fresh DB.
/// If pods are already populated (e.g. from a future DB-backed store), skips.
async fn seed_pods_on_startup(state: &Arc<AppState>) {
    // If pods already populated (future: DB-backed restore), skip
    if !state.pods.read().await.is_empty() {
        tracing::info!("Pods already populated, skipping auto-seed");
        return;
    }

    // (id, number, name, ip, mac)
    let pod_data: &[(&str, u32, &str, &str, &str)] = &[
        ("pod_1", 1, "Pod 1", "192.168.31.89", "30:56:0F:05:45:88"),
        ("pod_2", 2, "Pod 2", "192.168.31.33", "30:56:0F:05:46:53"),
        ("pod_3", 3, "Pod 3", "192.168.31.28", "30:56:0F:05:44:B3"),
        ("pod_4", 4, "Pod 4", "192.168.31.88", "30:56:0F:05:45:25"),
        ("pod_5", 5, "Pod 5", "192.168.31.86", "30:56:0F:05:44:B7"),
        ("pod_6", 6, "Pod 6", "192.168.31.87", "30:56:0F:05:45:6E"),
        ("pod_7", 7, "Pod 7", "192.168.31.38", "30:56:0F:05:44:B4"),
        ("pod_8", 8, "Pod 8", "192.168.31.91", "30:56:0F:05:46:C5"),
    ];

    let mut seeded = Vec::new();
    {
        let mut pods = state.pods.write().await;
        for &(id, number, name, ip, mac) in pod_data {
            let pod = PodInfo {
                id: id.to_string(),
                number,
                name: name.to_string(),
                ip_address: ip.to_string(),
                mac_address: Some(mac.to_string()),
                sim_type: SimType::AssettoCorsa,
                status: PodStatus::Idle,
                current_driver: None,
                current_session_id: None,
                last_seen: Some(chrono::Utc::now()),
                driving_state: None,
                billing_session_id: None,
                game_state: None,
                current_game: None,
                installed_games: vec![],
                screen_blanked: None,
                ffb_preset: None,
                freedom_mode: None,
                agent_timestamp: None, // Intentional default: server-side pod seeding has no agent clock
            };
            pods.insert(id.to_string(), pod.clone());
            seeded.push(pod);
        }
    }

    // BUG-01 FIX: Also seed pods into the SQLite database.
    // Previously only in-memory map was populated — kiosk queries DB directly,
    // so it saw empty pods table after server restart with fresh DB.
    for pod in &seeded {
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO pods (id, number, name, ip_address, sim_type, status, last_seen)
             VALUES (?, ?, ?, ?, 'assetto_corsa', 'idle', datetime('now'))"
        )
        .bind(&pod.id)
        .bind(pod.number as i64)
        .bind(&pod.name)
        .bind(&pod.ip_address)
        .execute(&state.db)
        .await;
    }

    // Broadcast individual pod updates
    for pod in &seeded {
        let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
    }

    // Broadcast full pod list
    let all_pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();
    let _ = state.dashboard_tx.send(DashboardEvent::PodList(all_pods));

    tracing::info!("Auto-seeded {} pods on startup (in-memory + DB)", seeded.len());
}

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
    if is_kiosk {
        if let Some(after_kiosk) = path_and_query.strip_prefix("/kiosk") {
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

/// Build security headers middleware via tower-helmet.
///
/// Sets CSP, X-Frame-Options (DENY), X-Content-Type-Options (nosniff),
/// and HSTS with a short max-age (300s / 5 minutes) for initial deploy safety.
/// The short HSTS avoids browser lockout during testing — increase after 1 week stable.
fn security_headers_layer() -> HelmetLayer {
    use tower_helmet::header::{
        ContentSecurityPolicy, StrictTransportSecurity, XFrameOptions,
    };

    let mut directives = std::collections::HashMap::new();
    directives.insert("default-src", vec!["'self'"]);
    directives.insert("script-src", vec!["'self'"]);
    directives.insert("style-src", vec!["'self'", "'unsafe-inline'"]);
    directives.insert("img-src", vec!["'self'", "data:"]);
    directives.insert("connect-src", vec!["'self'", "ws:", "wss:"]);
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
        .enable(tower_helmet::header::XContentTypeOptions::default())
        .enable(hsts);
    layer
}

/// Cache control middleware — matches cloud nginx strategy.
/// HTML/API: no-cache (always revalidate on update rollouts).
/// _next/static: immutable (content-hashed filenames, safe to cache forever).
/// Static assets (images): moderate cache with revalidation.
async fn cache_control_middleware(
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

fn cleanup_old_logs(log_dir: &std::path::Path) {
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(30 * 24 * 3600))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with(".jsonl") || name.contains(".jsonl.") || name.ends_with(".log") {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if modified < cutoff {
                            if std::fs::remove_file(&path).is_ok() {
                                eprintln!("Cleaned old log: {}", path.display());
                            }
                        }
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!(r#"
    ____                  ______            __             __
   / __ \____ _________  / ____/___  ____  / /__________  / /
  / /_/ / __ `/ ___/ _ \/ /   / __ \/ __ \/ __/ ___/ __ \/ /
 / _, _/ /_/ / /__/  __/ /___/ /_/ / / / / /_/ /  / /_/ / /
/_/ |_|\__,_/\___/\___/\____/\____/_/ /_/\__/_/   \____/_/

  Sim Racing Venue Management System
  by RacingPoint
"#);

    // Single-instance guard: prevent zombie racecontrol processes (same pattern as rc-agent 305638b)
    // When watchdog spawns a new instance while zombie holds ports, the mutex causes
    // the second instance to exit cleanly instead of crashing with os error 10048.
    #[cfg(windows)]
    let _mutex_guard = {
        use std::ffi::CString;
        let name = CString::new("Global\\RacingPoint_RaceControl_SingleInstance")
            .expect("mutex name contains no null bytes");
        let handle = unsafe {
            winapi::um::synchapi::CreateMutexA(
                std::ptr::null_mut(),
                1, // bInitialOwner = TRUE
                name.as_ptr(),
            )
        };
        if handle.is_null() || unsafe { winapi::um::errhandlingapi::GetLastError() } == 183 {
            // ERROR_ALREADY_EXISTS = 183
            eprintln!("racecontrol is already running. Exiting to prevent zombie.");
            if !handle.is_null() {
                unsafe { winapi::um::handleapi::CloseHandle(handle); }
            }
            std::process::exit(0);
        }
        handle // held until process exits → mutex released automatically
    };

    // Load config FIRST so MonitoringConfig is available for tracing init
    // Pre-init messages use eprintln! since tracing is not yet initialized
    eprintln!("Loading config...");
    let config = Config::load_or_default();

    // Initialize tracing — dual output: stdout (text) + rolling JSON log file
    // Config must be loaded before this point so error_rate thresholds are available
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    let log_dir = std::path::Path::new("logs");
    std::fs::create_dir_all(log_dir).ok();
    cleanup_old_logs(log_dir);

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("racecontrol-")
        .filename_suffix("jsonl")
        .build(log_dir)
        .expect("failed to build rolling file appender");
    let (non_blocking_file, _guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "racecontrol_crate=info,tower_http=info,debug=info".into());

    // Error rate monitoring — broadcast bridge from sync Layer to async alerters
    let (alert_tx, _) = tokio::sync::broadcast::channel::<()>(4);
    let email_alert_rx = alert_tx.subscribe();
    let wa_alert_rx = alert_tx.subscribe();
    let error_rate_config = ErrorRateConfig {
        threshold: config.monitoring.error_rate_threshold,
        window_secs: config.monitoring.error_rate_window_secs,
        cooldown_secs: config.monitoring.error_rate_cooldown_secs,
    };
    let error_count_layer = ErrorCountLayer::new(error_rate_config, alert_tx);

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_ansi(false)
                .with_writer(non_blocking_file),
        )
        .with(error_count_layer)
        .init();

    // Warn if default JWT secret is unchanged
    if config.auth.jwt_secret == "racingpoint-jwt-change-me-in-production" {
        tracing::warn!("Using default JWT secret! Set auth.jwt_secret in racecontrol.toml for production.");
    }
    tracing::info!("Venue: {} ({})", config.venue.name, config.venue.location);
    tracing::info!("Server: {}:{}", config.server.host, config.server.port);

    // Initialize database
    let pool = db::init_pool(&config.database.path).await?;

    // Extract monitoring/email config before config is moved into AppState
    let error_rate_email_enabled = config.monitoring.error_rate_email_enabled;
    let email_script_for_alerter = config.watchdog.email_script_path.clone();

    // Load encryption keys for PII field-level encryption
    let field_cipher = load_encryption_keys()
        .expect("Encryption keys required. Set RACECONTROL_ENCRYPTION_KEY and RACECONTROL_HMAC_KEY env vars (64 hex chars each). Generate with: openssl rand -hex 32");

    // Migrate existing plaintext PII to encrypted columns (idempotent)
    migrate_pii_encryption(&pool, &field_cipher).await
        .expect("PII migration failed");

    // Track admin PIN hash for rotation alerting (ADMIN-06)
    db::check_pin_rotation(&pool, &config).await;

    // Build application state
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let mut state = Arc::new(AppState::new(config, pool, field_cipher));

    // Phase 251: Initialize telemetry.db (separate from main racecontrol.db)
    {
        let telem_path = racecontrol_crate::telemetry_store::telemetry_db_path(
            &Arc::get_mut(&mut state).expect("no other Arc refs yet").config.database.path,
        );
        match racecontrol_crate::telemetry_store::init_telemetry_db(&telem_path).await {
            Ok(telem_pool) => {
                let writer_tx = racecontrol_crate::telemetry_store::spawn_writer(
                    telem_pool.clone(), None,
                );
                let inner = Arc::get_mut(&mut state).expect("no other Arc refs yet");
                inner.telemetry_writer_tx = Some(writer_tx);
                inner.telemetry_db = Some(telem_pool.clone());
                racecontrol_crate::telemetry_store::spawn_maintenance_scheduler(telem_pool.clone());
                // v29.0 Phase 5: Rule-based anomaly detection on hardware telemetry
                // Wire to self-healing availability map so anomalies update pod state
                let _anomaly_state = racecontrol_crate::maintenance_engine::spawn_anomaly_scanner_with_healing(
                    telem_pool,
                    Some(inner.pod_availability.clone()),
                );
                tracing::info!("Telemetry persistence enabled: {}", telem_path);
            }
            Err(e) => {
                tracing::error!("Failed to initialize telemetry.db: {} — persistence disabled", e);
            }
        }
    }

    // Phase 2 (v29.0): Initialize maintenance event tables in main DB
    if let Err(e) = racecontrol_crate::maintenance_store::init_maintenance_tables(
        &Arc::get_mut(&mut state).expect("no other Arc refs yet").db,
    ).await {
        tracing::error!("Failed to initialize maintenance tables: {e}");
    }

    // Phase 11 (v29.0): Initialize business metrics tables
    if let Err(e) = racecontrol_crate::maintenance_store::init_business_tables(
        &Arc::get_mut(&mut state).expect("no other Arc refs yet").db,
    ).await {
        tracing::error!("Failed to initialize business metrics tables: {e}");
    }

    // Phase 13-14 (v29.0): Initialize HR + attendance tables
    if let Err(e) = racecontrol_crate::maintenance_store::init_hr_tables(
        &Arc::get_mut(&mut state).expect("no other Arc refs yet").db,
    ).await {
        tracing::error!("Failed to initialize HR tables: {e}");
    }

    // Phase 26 (v29.0): Spawn hourly business aggregator (billing+cafe -> daily_business_metrics)
    {
        let agg_db = Arc::get_mut(&mut state).expect("no other Arc refs yet").db.clone();
        racecontrol_crate::business_aggregator::spawn_business_aggregator(agg_db);
    }

    // Phase 28 (v29.0): Initialize feedback loop tables (prediction outcomes, admin overrides)
    if let Err(e) = racecontrol_crate::feedback_loop::init_feedback_tables(
        &Arc::get_mut(&mut state).expect("no other Arc refs yet").db,
    ).await {
        tracing::error!("Failed to initialize feedback tables: {e}");
    }

    // Phase 30 (v29.0): Initialize pricing proposal tables
    if let Err(e) = racecontrol_crate::pricing_bridge::init_pricing_tables(
        &Arc::get_mut(&mut state).expect("no other Arc refs yet").db,
    ).await {
        tracing::error!("Failed to initialize pricing tables: {e}");
    }

    // Phase 30 (v29.0): alert checker spawned below after all Arc::get_mut calls

    // Phase 253: Spawn driver rating worker and set up backfill
    {
        let inner = Arc::get_mut(&mut state).expect("no other Arc refs yet");
        let rating_venue_id = inner.config.venue.venue_id.clone();
        let rating_tx = racecontrol_crate::driver_rating::spawn_rating_worker(inner.db.clone(), rating_venue_id.clone());
        inner.rating_tx = Some(rating_tx);
        let backfill_db = inner.db.clone();
        tokio::spawn(async move {
            racecontrol_crate::driver_rating::backfill_ratings(backfill_db, rating_venue_id).await;
        });
    }

    // Phase 307: Load last audit chain hash from DB so hash chain continues correctly after restart.
    // If no hashed entries exist yet (fresh DB or pre-migration), stays at GENESIS.
    {
        let db = Arc::get_mut(&mut state).expect("no other Arc refs yet").db.clone();
        let last_hash: Option<String> = sqlx::query_scalar(
            "SELECT entry_hash FROM pod_activity_log WHERE entry_hash IS NOT NULL ORDER BY timestamp DESC LIMIT 1"
        )
        .fetch_optional(&db)
        .await
        .ok()
        .flatten();

        if let Some(hash) = last_hash {
            let inner = Arc::get_mut(&mut state).expect("no other Arc refs yet");
            if let Ok(mut guard) = inner.audit_last_hash.lock() {
                *guard = hash;
            }
            tracing::info!("Phase 307: Audit hash chain resumed from existing entry");
        } else {
            tracing::info!("Phase 307: Audit hash chain starting from GENESIS");
        }
    }

    // Auto-seed all 8 pods on startup so kiosk is never left with empty pod list
    // after server restart with fresh DB (BUG-01)
    seed_pods_on_startup(&state).await;

    // v22.0 Phase 177: Load feature flags into in-memory cache and initialize config_push_seq
    state.load_feature_flags().await;

    // v22.0 Phase 179: Check for interrupted OTA pipeline on startup
    racecontrol_crate::ota_pipeline::check_interrupted_pipeline();

    // Phase 30 (v29.0): Spawn business alert checker (every 30 min)
    // Wired to WhatsApp + dashboard alert delivery
    racecontrol_crate::alert_engine::spawn_alert_checker(state.clone());

    // v34.0 Phase 285: Metrics TSDB -- async ingestion pipeline + rollup/purge
    let metrics_tx = racecontrol_crate::metrics_tsdb::spawn_metrics_ingestion(state.db.clone());
    racecontrol_crate::metrics_tsdb::spawn_rollup_and_purge(state.db.clone());
    tracing::info!("Metrics TSDB ingestion + rollup/purge tasks spawned");
    racecontrol_crate::metrics_producers::spawn_metric_producers(state.clone(), metrics_tx);
    tracing::info!("Metrics producers spawned (ws_connections, game_sessions, pod_health, billing_revenue)");

    // Spawn error rate alerter task — sends to both James and Uday on error spikes
    if error_rate_email_enabled {
        let email_script = email_script_for_alerter;
        let recipients = vec![
            "james@racingpoint.in".to_string(),
            "usingh@racingpoint.in".to_string(),
        ];
        tokio::spawn(error_rate_alerter_task(email_alert_rx, email_script, recipients));
    }

    // Spawn WhatsApp P0 alerter task
    if state.config.alerting.enabled {
        let wa_state = state.clone();
        tokio::spawn(racecontrol_crate::whatsapp_alerter::whatsapp_alerter_task(
            wa_state,
            wa_alert_rx,
        ));
    }

    // Spawn metric alert evaluation task
    if !state.config.alert_rules.is_empty() {
        let alert_state = state.clone();
        tokio::spawn(racecontrol_crate::metric_alerts::metric_alert_task(alert_state));
        tracing::info!(target: "startup", "metric alert task spawned ({} rules)", state.config.alert_rules.len());
    }

    // Spawn policy engine evaluation task (Phase 299 — re-loads rules each cycle from DB)
    let policy_state = state.clone();
    tokio::spawn(racecontrol_crate::policy_engine::policy_engine_task(policy_state));

    // Spawn notification outbox worker (UX-01: durable retry with exponential backoff)
    {
        let notif_state = state.clone();
        tokio::spawn(racecontrol_crate::notification_outbox::notification_worker_task(notif_state));
    }

    // UX-08: Spawn virtual queue expire task (expires 'called' entries after 10 minutes, runs every 5 min)
    {
        let queue_db = state.db.clone();
        tokio::spawn(racecontrol_crate::api::routes::queue_expire_task(queue_db));
    }

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

    // FSM-10: Detect orphaned sessions on startup (sessions with stale heartbeat)
    billing::detect_orphaned_sessions_on_startup(&state).await;

    // Load billing rate tiers from DB into cache
    billing::refresh_rate_tiers(&state).await;

    // Spawn billing tick loop (1 second interval, refresh rates every 60s)
    // MMA-Iter3: Wrap in restart loop so panics don't silently kill billing
    let tick_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("billing-tick task started (1s interval)");
        loop {
            let state = tick_state.clone();
            let handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                let mut refresh_counter: u32 = 0;
                loop {
                    interval.tick().await;
                    billing::tick_all_timers(&state).await;
                    refresh_counter += 1;
                    if refresh_counter >= 60 {
                        refresh_counter = 0;
                        billing::refresh_rate_tiers(&state).await;
                    }
                }
            });
            if let Err(e) = handle.await {
                tracing::error!("CRITICAL: billing-tick task panicked: {:?} — restarting in 1s", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    });

    // Spawn billing DB sync loop (5 second interval)
    let sync_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("billing-db-sync task started (5s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            billing::sync_timers_to_db(&sync_state).await;
        }
    });

    // Spawn staggered timer persistence loop (RESIL-02 + FSM-09)
    // Each pod persists elapsed_seconds at a different second offset within the minute:
    // Pod 1 at :07, Pod 2 at :14, Pod 3 at :21, Pod 4 at :28,
    // Pod 5 at :35, Pod 6 at :42, Pod 7 at :49, Pod 8 at :56
    // Formula: Pod N writes at second (N * 7) % 60
    let persist_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("timer-persist task started (60s staggered by pod index)");
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut second_counter: u64 = 0;
        loop {
            interval.tick().await;
            second_counter += 1;
            let second_in_minute = second_counter % 60;

            // Check if any pod should write this second
            for pod_num in 1u32..=8 {
                if (pod_num as u64 * 7) % 60 == second_in_minute {
                    billing::persist_timer_state(&persist_state, Some(pod_num)).await;
                }
            }
        }
    });

    // Spawn orphan detection background task (RESIL-03: every 5 minutes)
    let orphan_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("orphan-detector task started (300s interval)");
        // Initial delay: wait 5 minutes before first background check
        // (startup check already ran — avoid duplicate alerts for same orphans)
        tokio::time::sleep(Duration::from_secs(300)).await;
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            billing::detect_orphaned_sessions_background(&orphan_state).await;
        }
    });

    // Spawn wallet reconciliation background task (FATM-12: every 30 minutes)
    billing::spawn_reconciliation_job(state.clone());

    // BILL-03: Spawn PWA game request TTL cleanup task (every 60 seconds)
    billing::spawn_cleanup_expired_game_requests(state.clone());

    // FATM-08: Spawn coupon TTL expiry task (every 60s, 120s initial delay)
    billing::spawn_coupon_ttl_expiry_job(state.clone());

    // Spawn data retention background task (LEGAL-08: daily, 1-hour initial delay)
    // Anonymizes drivers inactive for > pii_inactive_months (default 24 months).
    // Financial records are never touched (Income Tax Act: 8-year retention).
    {
        let retention_state = state.clone();
        tokio::spawn(async move {
            api::routes::spawn_data_retention_job(retention_state).await;
        });
    }

    // Spawn game health check loop (5 second interval)
    let game_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("game-health-check task started (5s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            game_launcher::check_game_health(&game_state).await;
        }
    });

    // Spawn AC server health check loop (5 second interval)
    let ac_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("ac-server-health task started (5s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            ac_server::check_ac_server_health(&ac_state).await;
        }
    });

    // Spawn auth token expiry loop (30 second interval)
    let auth_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("auth-token-expiry task started (30s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            auth::expire_stale_tokens(&auth_state).await;
        }
    });

    // Spawn pod reservation expiry loop (30 second interval)
    let res_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("pod-reservation-expiry task started (30s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            pod_reservation::expire_idle_reservations(&res_state).await;
        }
    });

    // Spawn camera control tick loop (2 second interval)
    let cam_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("camera-control-tick task started (2s interval)");
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

    // v29.0 Phase 35: Spawn data collector + RUL threshold checks (15-min interval)
    if let Some(telem_pool) = state.telemetry_db.clone() {
        racecontrol_crate::data_collector::spawn_data_collector(state.db.clone(), telem_pool);
    } else {
        tracing::warn!("Data collector skipped — telemetry DB not initialized");
    }

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

    // Spawn SQLite backup pipeline (hourly VACUUM INTO, rotation, staleness alert)
    backup_pipeline::spawn(state.clone());
    tracing::info!(target: "startup", "backup pipeline spawned");

    // Spawn event archive pipeline (hourly tick: JSONL export, 90-day purge, nightly SCP)
    event_archive::spawn(state.clone());
    tracing::info!(target: "startup", "event_archive pipeline spawned");

    // Spawn psychology notification dispatcher (drains nudge_queue, routes to channels)
    psychology::spawn_dispatcher(state.clone());

    // Spawn UDP heartbeat listener (fast liveness detection alongside WebSocket)
    udp_heartbeat::spawn(state.clone());

    // Spawn fleet health probe loop (15s interval, HTTP :8090/health on each registered pod)
    fleet_health::start_probe_loop(state.clone());

    // Spawn deployment awareness (60s interval, fleet version consistency + crash detection)
    deploy_awareness::spawn(state.clone());

    // Spawn app health monitor (30s interval, probes admin/kiosk/web health endpoints)
    app_health_monitor::spawn(state.clone());

    // Spawn synthetic transaction monitor (5min interval, golden-path API validation)
    racecontrol_crate::synthetic_monitor::spawn(state.clone());

    // Spawn Meshed Intelligence promotion pipeline (60s: promote candidates, detect patterns, expire stale)
    racecontrol_crate::promotion::spawn(state.clone());

    // Spawn server self-diagnostics (MMA consensus: WS drift, split-brain, DB health — 60s interval)
    racecontrol_crate::server_diagnostics::spawn(state.clone());

    // Spawn server-side process guard (monitors server .23 for unauthorized processes)
    process_guard::spawn_server_guard(state.clone());

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
                        if let Err(e) = axum::serve(ts_listener, relay_router).await {
                            tracing::error!("Bono relay server on {} exited with error: {e}", ts_addr);
                        }
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
        .nest("/api/v1", api::routes::api_routes(state.clone()))
        // WebSocket endpoints
        .route("/ws/agent", get(ws::agent_ws))
        .route("/ws/dashboard", get(ws::dashboard_ws))
        .route("/ws/ai", get(ws::ai_ws))
        // Registration page (standalone HTML for QR code walk-in flow)
        .route("/register", get(|| async {
            axum::response::Html(include_str!("../../../assets/register.html"))
        }))
        // Portal — single URL linking to all apps (kiosk, admin, POS, web)
        .route("/portal", get(|| async {
            axum::response::Html(include_str!("../../../assets/portal.html"))
        }))
        // Status dashboard — visual UI for health + fleet + services
        .route("/status", get(|| async {
            axum::response::Html(include_str!("../../../assets/status.html"))
        }))
        // Redirects: common wrong URLs → correct destinations
        // Staff/POS might type these directly — redirect to the right app
        // MMA consensus: use 307 (not 308) to avoid permanent cache of hardcoded IP
        .route("/admin", get(|| async { axum::response::Redirect::temporary("http://192.168.31.23:3201/") }))
        .route("/admin/", get(|| async { axum::response::Redirect::temporary("http://192.168.31.23:3201/") }))
        .route("/pos", get(|| async { axum::response::Redirect::permanent("/billing") }))
        .route("/dashboard", get(|| async { axum::response::Redirect::permanent("/billing") }))
        .route("/staff", get(|| async { axum::response::Redirect::permanent("/kiosk/staff") }))
        .route("/spectator", get(|| async { axum::response::Redirect::permanent("/kiosk/spectator") }))
        .route("/control", get(|| async { axum::response::Redirect::permanent("/kiosk/control") }))
        .route("/fleet", get(|| async { axum::response::Redirect::permanent("/kiosk/fleet") }))
        .route("/cameras", get(|| async { axum::response::Redirect::permanent("/kiosk/fleet") }))
        .route("/book", get(|| async { axum::response::Redirect::permanent("/kiosk/book") }))
        // MMA consensus: /kiosk without trailing slash misses proxy, add redirect
        .route("/kiosk", get(|| async { axum::response::Redirect::temporary("/kiosk/") }))
        // Root → portal directory page (so 192.168.31.23 shows all links)
        .route("/", get(|| async { axum::response::Redirect::temporary("/portal") }))
        // Static file serving for cafe item images
        .nest_service("/static/cafe-images", tower_http::services::ServeDir::new("./data/cafe-images"))
        // Reverse proxy: kiosk UI + Next.js assets → localhost:3300
        .fallback(kiosk_proxy)
        .layer(axum_mw::from_fn(jwt_error_to_401))
        .layer(security_headers_layer())
        .layer(axum_mw::from_fn(cache_control_middleware))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
                    let origin = origin.to_str().unwrap_or("");
                    // SEC-P1-5: Restrict CORS to known service origins only
                    // MMA iter2: pod kiosks need CORS too (browser → server API calls)
                    origin.starts_with("http://localhost:")
                        || origin.starts_with("https://localhost:")
                        || origin.starts_with("http://127.0.0.1:")
                        || origin.starts_with("https://127.0.0.1:")
                        || origin.starts_with("http://192.168.31.23:")  // server
                        || origin.starts_with("http://192.168.31.27:")  // james
                        || origin.starts_with("http://192.168.31.20:")  // POS
                        || origin.starts_with("http://192.168.31.89:")  // pod 1
                        || origin.starts_with("http://192.168.31.33:")  // pod 2
                        || origin.starts_with("http://192.168.31.28:")  // pod 3
                        || origin.starts_with("http://192.168.31.88:")  // pod 4
                        || origin.starts_with("http://192.168.31.86:")  // pod 5
                        || origin.starts_with("http://192.168.31.87:")  // pod 6
                        || origin.starts_with("http://192.168.31.38:")  // pod 7
                        || origin.starts_with("http://192.168.31.91:")  // pod 8
                        || origin.starts_with("http://kiosk.rp")
                        || origin.starts_with("https://kiosk.rp")
                        || origin == "https://app.racingpoint.cloud"
                }))
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE, Method::OPTIONS])
                .allow_headers(tower_http::cors::Any)
                .allow_credentials(false)
        )
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024)) // 1MB request body limit
        .layer(TraceLayer::new_for_http())
        .layer(axum_mw::from_fn(classify_source_middleware))
        .with_state(state.clone());

    // Start HTTP server
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("RaceControl HTTP on http://{}", bind_addr);
    tracing::info!("Dashboard:    http://{}/", bind_addr);
    tracing::info!("API:          http://{}/api/v1/health", bind_addr);
    tracing::info!("Agent WS:     ws://{}/ws/agent", bind_addr);
    tracing::info!("Dashboard WS: ws://{}/ws/dashboard", bind_addr);
    tracing::info!("AI WS:        ws://{}/ws/ai", bind_addr);

    // Start HTTPS server (if tls_port configured -- legacy one-way TLS path)
    if let Some(tls_port) = state.config.server.tls_port {
        let tls_config = tls::load_or_generate_rustls_config(
            &state.config.server.host,
            state.config.server.cert_path.as_deref(),
            state.config.server.key_path.as_deref(),
        ).await?;
        let https_addr: std::net::SocketAddr = format!("{}:{}", state.config.server.host, tls_port)
            .parse()
            .expect("invalid tls_port address");
        let https_app = app.clone();
        tracing::info!("RaceControl HTTPS on https://{}", https_addr);
        tokio::spawn(async move {
            if let Err(e) = axum_server::bind_rustls(https_addr, tls_config)
                .serve(https_app.into_make_service())
                .await
            {
                tracing::error!("HTTPS listener failed: {}", e);
            }
        });
    }

    // Phase 305: Venue CA mTLS on main :8080 listener.
    // When server.tls.enabled = true, binds with TLS (one-way or mTLS per require_client_cert).
    // When false (default), falls through to plain HTTP (backward compatible).
    // Tailscale relay listener always stays plain HTTP -- it uses a separate bind IP.
    if state.config.server.tls.enabled {
        let mtls_cfg = tls::load_mtls_config(&state.config.server.tls).await?;
        let mode = if state.config.server.tls.require_client_cert { "mTLS" } else { "TLS (one-way)" };
        tracing::info!("RaceControl {} on https://{}", mode, bind_addr);
        axum_server::bind_rustls(listener.local_addr()?, mtls_cfg)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await?;
    } else {
        // HTTP listener (blocking -- keeps main alive)
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    }

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
