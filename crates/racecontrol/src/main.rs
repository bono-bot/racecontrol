use axum::Router;
use axum::routing::get;
use axum::middleware as axum_mw;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{CorsLayer, AllowOrigin};
use axum::http::{HeaderValue, Method};
use tower_http::trace::TraceLayer;

use racecontrol_crate::config::Config;
use racecontrol_crate::crypto::encryption::load_encryption_keys;
use racecontrol_crate::db::migrate_pii_encryption;
use racecontrol_crate::network_source::classify_source_middleware;
use racecontrol_crate::tls;
use racecontrol_crate::state::AppState;
use racecontrol_crate::{
    ac_server, api, billing, bono_relay, db, ws,
};

mod startup;
mod middleware;
mod background_tasks;

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
    let (_guard, email_alert_rx, wa_alert_rx) = startup::init_tracing(&config);

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
    init_telemetry(&mut state).await;

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
    startup::seed_pods_on_startup(&state).await;

    // Phase 335: Load pre-extracted track outlines for spectator circuit viewer
    state.track_outlines.load_preextracted();

    // v22.0 Phase 177: Load feature flags into in-memory cache and initialize config_push_seq
    state.load_feature_flags().await;

    // v22.0 Phase 179: Check for interrupted OTA pipeline on startup
    racecontrol_crate::ota_pipeline::check_interrupted_pipeline();

    // Phase 30 (v29.0): Spawn business alert checker (every 30 min)
    racecontrol_crate::alert_engine::spawn_alert_checker(state.clone());

    // First-boot email test: verify Gmail OAuth works on initial setup
    startup::maybe_send_first_boot_email(&state).await;

    // Clean up orphaned acServer processes from previous run
    match ac_server::cleanup_orphaned_sessions(&state.db, &state.port_allocator).await {
        Ok(0) => tracing::info!("No orphaned AC sessions found"),
        Ok(n) => tracing::warn!("Cleaned up {} orphaned AC sessions on startup", n),
        Err(e) => tracing::error!("Failed to clean up orphaned sessions: {}", e),
    }

    // Recover any active billing sessions from DB
    billing::recover_active_sessions(&state).await?;

    // GLD-C-04 Phase 363 (P0-3 fix): Patch grace-window fields onto timers
    if let Err(e) = billing::hydrate_grace_fields_from_db(&state.billing, &state.db).await {
        tracing::error!(error = %e, "GLD-C-04: failed to hydrate grace fields on startup");
    }

    // FSM-10: Detect orphaned sessions on startup (sessions with stale heartbeat)
    billing::detect_orphaned_sessions_on_startup(&state).await;

    // Load billing rate tiers from DB into cache
    billing::refresh_rate_tiers(&state).await;

    // Spawn all background tasks
    background_tasks::spawn_all(
        state.clone(),
        email_alert_rx,
        wa_alert_rx,
        error_rate_email_enabled,
        email_script_for_alerter,
    );

    // Bind Bono relay endpoint on Tailscale IP (optional — only if configured)
    if let Some(ts_ip) = state.config.bono.tailscale_bind_ip.clone()
        && state.config.bono.enabled {
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
                    }
                }
            });
        }

    // Build router
    let app = build_router(state.clone());

    // Start HTTP server
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("RaceControl HTTP on http://{}", bind_addr);
    tracing::info!("Dashboard:    http://{}/", bind_addr);
    tracing::info!("API:          http://{}/api/v1/health", bind_addr);
    tracing::info!("Agent WS:     ws://{}/ws/agent", bind_addr);
    tracing::info!("Dashboard WS: ws://{}/ws/dashboard", bind_addr);
    tracing::info!("AI WS:        ws://{}/ws/ai", bind_addr);
    tracing::info!("Spectator WS: ws://{}/ws/spectator", bind_addr);

    // mDNS service advertiser — lets pods discover this server without hardcoded IPs.
    let _mdns_daemon = if state.config.server.mdns_enabled {
        racecontrol_crate::mdns::start_advertiser(
            state.config.server.port,
            env!("GIT_HASH"),
            &state.config.venue.venue_id,
        )
    } else {
        tracing::info!("mDNS advertiser disabled by config (server.mdns_enabled = false)");
        None
    };

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

/// Initialize telemetry.db (separate from main racecontrol.db).
async fn init_telemetry(state: &mut Arc<AppState>) {
    let telem_path = racecontrol_crate::telemetry_store::telemetry_db_path(
        &Arc::get_mut(state).expect("no other Arc refs yet").config.database.path,
    );
    match racecontrol_crate::telemetry_store::init_telemetry_db(&telem_path).await {
        Ok(telem_pool) => {
            let writer_tx = racecontrol_crate::telemetry_store::spawn_writer(
                telem_pool.clone(), None,
            );
            let inner = Arc::get_mut(state).expect("no other Arc refs yet");
            inner.telemetry_writer_tx = Some(writer_tx);
            inner.telemetry_db = Some(telem_pool.clone());
            racecontrol_crate::telemetry_store::spawn_maintenance_scheduler(telem_pool.clone());
            // v29.0 Phase 5: Rule-based anomaly detection on hardware telemetry
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

/// Build the Axum router with all routes, middleware, and layers.
fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        // API routes
        .nest("/api/v1", api::routes::api_routes(state.clone()))
        // WebSocket endpoints
        .route("/ws/agent", get(ws::agent_ws))
        .route("/ws/dashboard", get(ws::dashboard_ws))
        .route("/ws/ai", get(ws::ai_ws))
        .route("/ws/spectator", get(ws::spectator_ws))
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
        .route("/admin", get(|| async { axum::response::Redirect::temporary("http://192.168.31.23:3201/") }))
        .route("/admin/", get(|| async { axum::response::Redirect::temporary("http://192.168.31.23:3201/") }))
        .route("/pos", get(|| async { axum::response::Redirect::temporary("/billing") }))
        .route("/dashboard", get(|| async { axum::response::Redirect::temporary("/billing") }))
        .route("/staff", get(|| async { axum::response::Redirect::temporary("/kiosk/staff") }))
        .route("/spectator", get(|| async { axum::response::Redirect::temporary("/kiosk/spectator") }))
        .route("/control", get(|| async { axum::response::Redirect::temporary("/kiosk/control") }))
        .route("/fleet", get(|| async { axum::response::Redirect::temporary("http://192.168.31.23:3200/fleet") }))
        .route("/cameras", get(|| async { axum::response::Redirect::temporary("http://192.168.31.23:3200/cameras") }))
        // Catch browser-cached 308s from old /cameras→/kiosk/fleet redirect
        .route("/kiosk/fleet", get(|| async { axum::response::Redirect::temporary("http://192.168.31.23:3200/cameras") }))
        .route("/book", get(|| async { axum::response::Redirect::temporary("/kiosk/book") }))
        // MMA consensus: /kiosk without trailing slash misses proxy, add redirect
        .route("/kiosk", get(|| async { axum::response::Redirect::temporary("/kiosk/") }))
        // Root → portal directory page (so 192.168.31.23 shows all links)
        .route("/", get(|| async { axum::response::Redirect::temporary("/portal") }))
        // Static file serving for cafe item images
        .nest_service("/static/cafe-images", tower_http::services::ServeDir::new("./data/cafe-images"))
        // Reverse proxy: kiosk UI + Next.js assets → localhost:3300
        .fallback(middleware::kiosk_proxy)
        .layer(axum_mw::from_fn(middleware::jwt_error_to_401))
        .layer(middleware::security_headers_layer())
        .layer(axum_mw::from_fn(middleware::cache_control_middleware))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
                    let origin = origin.to_str().unwrap_or("");
                    let host_port = origin
                        .strip_prefix("http://")
                        .or_else(|| origin.strip_prefix("https://"))
                        .unwrap_or("");
                    let host = host_port.split(':').next().unwrap_or("");

                    host.starts_with("192.168.31.")
                        || host == "localhost"
                        || host == "127.0.0.1"
                        || host.starts_with("100.")
                        || host.starts_with("kiosk.rp")
                        || origin == "https://app.racingpoint.cloud"
                        || origin == "https://admin.racingpoint.cloud"
                        || origin == "https://racingpoint.cloud"
                }))
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE, Method::OPTIONS])
                .allow_headers(tower_http::cors::Any)
                .allow_credentials(false)
        )
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024)) // 1MB request body limit
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<axum::body::Body>| {
                    let correlation_id = uuid::Uuid::new_v4().to_string();
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        route = %request.uri().path(),
                        correlation_id = %correlation_id,
                    )
                })
                .on_request(|request: &axum::http::Request<axum::body::Body>, _span: &tracing::Span| {
                    tracing::info!(
                        target: "admin_api",
                        method = %request.method(),
                        route = %request.uri().path(),
                        "request_started"
                    );
                })
                .on_response(|response: &axum::http::Response<axum::body::Body>, latency: std::time::Duration, _span: &tracing::Span| {
                    tracing::info!(
                        target: "admin_api",
                        status = response.status().as_u16(),
                        latency_ms = latency.as_millis() as u64,
                        "request_completed"
                    );
                })
        )
        .layer(axum_mw::from_fn(classify_source_middleware))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use crate::middleware::backend_unavailable_page;

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
