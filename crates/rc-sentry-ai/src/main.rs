mod config;
mod detection;
mod frame;
mod health;
mod privacy;
mod relay;
mod stream;

use std::sync::Arc;

use config::Config;
use frame::FrameBuffer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load config from CLI arg or default path
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r"C:\RacingPoint\rc-sentry-ai.toml".to_string());

    let config = Config::load(&config_path)?;

    tracing::info!(
        cameras = config.cameras.len(),
        relay_rtsp = %config.relay.rtsp_base,
        relay_api = %config.relay.api_url,
        port = config.service.port,
        "rc-sentry-ai starting"
    );

    let frame_buf = FrameBuffer::new();

    // Initialize detection stats (shared with health endpoint regardless of detection enabled)
    let detection_stats = Arc::new(detection::pipeline::DetectionStats::new());

    // Initialize SCRFD detector and spawn per-camera detection tasks
    if config.detection.enabled {
        match detection::scrfd::ScrfdDetector::new(&config.detection.model_path) {
            Ok(detector) => {
                let detector = Arc::new(detector);
                tracing::info!(
                    model = %config.detection.model_path,
                    confidence = config.detection.confidence_threshold,
                    "SCRFD detector initialized with CUDA EP"
                );

                // Spawn one detection task per camera
                for camera in config.cameras.iter() {
                    let cam_name = camera.name.clone();
                    let buf = frame_buf.clone();
                    let det = Arc::clone(&detector);
                    let conf = config.detection.confidence_threshold;
                    let stats = Arc::clone(&detection_stats);
                    tokio::spawn(async move {
                        detection::pipeline::run(cam_name, buf, det, conf, stats).await;
                    });
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to initialize SCRFD detector, detection disabled");
            }
        }
    } else {
        tracing::info!("face detection disabled in config");
    }

    // Initialize audit writer (single-writer pattern for Windows file locking)
    let (audit_writer, _audit_handle) = privacy::audit::AuditWriter::new(
        config.privacy.audit_log_path.clone(),
    );
    let audit_writer = Arc::new(audit_writer);

    // Spawn one task per camera for independent RTSP streaming
    for camera in config.cameras.iter() {
        let cam = camera.clone();
        let rtsp_base = config.relay.rtsp_base.clone();
        let buf = frame_buf.clone();
        tokio::spawn(async move {
            stream::camera_loop(cam, rtsp_base, buf).await;
        });
    }

    // Health endpoint (Plan 03)
    let state = Arc::new(health::AppState {
        frame_buf: frame_buf.clone(),
        relay_api_url: config.relay.api_url.clone(),
        start_time: std::time::Instant::now(),
        detection_stats: Arc::clone(&detection_stats),
    });

    // Spawn retention purge task (hourly)
    {
        let audit = audit_writer.clone();
        let retention_days = config.privacy.retention_days;
        tokio::spawn(async move {
            privacy::retention::retention_purge_task(retention_days, audit).await;
        });
    }

    let app = health::health_router(state)
        .merge(health::privacy_router(audit_writer.clone()));
    let addr = format!("{}:{}", config.service.host, config.service.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("rc-sentry-ai health endpoint listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
