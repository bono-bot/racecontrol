mod api;
mod billing;
mod config;
mod db;
mod state;
mod ws;

use axum::Router;
use axum::routing::get;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use config::Config;
use state::AppState;

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

    // Build router
    let app = Router::new()
        // API routes
        .nest("/api/v1", api::routes::api_routes())
        // WebSocket endpoints
        .route("/ws/agent", get(ws::agent_ws))
        .route("/ws/dashboard", get(ws::dashboard_ws))
        // Health check at root
        .route("/", get(|| async {
            axum::Json(serde_json::json!({
                "name": "RaceControl",
                "status": "running",
                "version": env!("CARGO_PKG_VERSION"),
            }))
        }))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("RaceControl listening on http://{}", bind_addr);
    tracing::info!("Dashboard:   http://{}/", bind_addr);
    tracing::info!("API:         http://{}/api/v1/health", bind_addr);
    tracing::info!("Agent WS:    ws://{}/ws/agent", bind_addr);
    tracing::info!("Dashboard WS: ws://{}/ws/dashboard", bind_addr);

    axum::serve(listener, app).await?;

    Ok(())
}
