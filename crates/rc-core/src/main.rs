mod ac_camera;
mod ac_server;
mod action_queue;
mod ai;
mod api;
mod auth;
mod billing;
mod catalog;
mod cloud_sync;
mod config;
mod db;
mod error_aggregator;
mod friends;
mod game_launcher;
mod multiplayer;
mod lap_tracker;
mod pod_healer;
mod pod_monitor;
mod pod_reservation;
mod remote_terminal;
mod scheduler;
mod state;
mod wallet;
mod udp_heartbeat;
mod wol;
mod ws;

use axum::Router;
use axum::routing::get;
use axum::middleware as axum_mw;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{CorsLayer, AllowOrigin};
use axum::http::{HeaderValue, Method};
use tower_http::trace::TraceLayer;

use config::Config;
use state::AppState;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rc_core=info,tower_http=info".into()),
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

    // Recover any active billing sessions from DB
    billing::recover_active_sessions(&state).await?;

    // Spawn billing tick loop (1 second interval)
    let tick_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            billing::tick_all_timers(&tick_state).await;
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

    // Spawn cloud sync (pulls customer data from cloud rc-core)
    cloud_sync::spawn(state.clone());

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
        .layer(axum_mw::from_fn(jwt_error_to_401))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
                    let origin = origin.to_str().unwrap_or("");
                    origin.starts_with("http://localhost:")
                        || origin.starts_with("http://127.0.0.1:")
                        || origin.starts_with("http://192.168.31.")
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
