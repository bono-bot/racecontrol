mod config;
mod detection;
mod enrollment;
mod frame;
mod health;
mod privacy;
mod recognition;
mod relay;
mod stream;

use std::sync::Arc;

use config::Config;
use frame::FrameBuffer;
use recognition::arcface::ArcfaceRecognizer;
use recognition::gallery::Gallery;
use recognition::quality::QualityGates;
use recognition::tracker::FaceTracker;

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

    // Initialize recognition components (if enabled)
    let recognizer: Option<Arc<ArcfaceRecognizer>> = if config.recognition.enabled {
        match ArcfaceRecognizer::new(&config.recognition.model_path) {
            Ok(r) => {
                tracing::info!(
                    model = %config.recognition.model_path,
                    threshold = config.recognition.similarity_threshold,
                    "ArcFace recognizer initialized"
                );
                Some(Arc::new(r))
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to initialize ArcFace recognizer, recognition disabled");
                None
            }
        }
    } else {
        tracing::info!("face recognition disabled in config");
        None
    };

    // Initialize gallery from SQLite (if recognition enabled)
    let gallery = if config.recognition.enabled {
        let db_path = config.recognition.gallery_db_path.clone();
        let threshold = config.recognition.similarity_threshold;
        match tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<recognition::types::GalleryEntry>> {
            let conn = rusqlite::Connection::open(&db_path)?;
            recognition::db::create_tables(&conn)?;
            let entries = recognition::db::load_gallery(&conn)?;
            Ok(entries)
        }).await? {
            Ok(entries) => {
                tracing::info!(entries = entries.len(), "face gallery loaded from SQLite");
                Arc::new(Gallery::new(entries, threshold))
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to load face gallery, starting empty");
                Arc::new(Gallery::new(vec![], config.recognition.similarity_threshold))
            }
        }
    } else {
        Arc::new(Gallery::new(vec![], config.recognition.similarity_threshold))
    };

    // Initialize face tracker
    let tracker = Arc::new(FaceTracker::new(config.recognition.tracker_cooldown_secs));

    // Initialize quality gates from config
    let quality_gates_config = QualityGates {
        min_face_size: config.recognition.min_face_size,
        min_laplacian_var: config.recognition.min_laplacian_var,
        max_yaw_degrees: config.recognition.max_yaw_degrees,
    };

    // Spawn periodic tracker cleanup (every 5 minutes)
    {
        let tracker_cleanup = Arc::clone(&tracker);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                interval.tick().await;
                tracker_cleanup.cleanup();
            }
        });
    }

    // Initialize SCRFD detector (shared between detection pipeline and enrollment)
    let shared_detector: Option<Arc<detection::scrfd::ScrfdDetector>> =
        if config.detection.enabled {
            match detection::scrfd::ScrfdDetector::new(&config.detection.model_path) {
                Ok(detector) => {
                    tracing::info!(
                        model = %config.detection.model_path,
                        confidence = config.detection.confidence_threshold,
                        "SCRFD detector initialized with CUDA EP"
                    );
                    Some(Arc::new(detector))
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to initialize SCRFD detector, detection disabled");
                    None
                }
            }
        } else {
            tracing::info!("face detection disabled in config");
            None
        };

    // Spawn per-camera detection tasks (if detector available)
    if let Some(ref detector) = shared_detector {
        for camera in config.cameras.iter() {
            let cam_name = camera.name.clone();
            let buf = frame_buf.clone();
            let det = Arc::clone(detector);
            let conf = config.detection.confidence_threshold;
            let stats = Arc::clone(&detection_stats);
            let rec = recognizer.clone();
            let qg = QualityGates {
                min_face_size: quality_gates_config.min_face_size,
                min_laplacian_var: quality_gates_config.min_laplacian_var,
                max_yaw_degrees: quality_gates_config.max_yaw_degrees,
            };
            let gal = Arc::clone(&gallery);
            let trk = Arc::clone(&tracker);
            tokio::spawn(async move {
                detection::pipeline::run(
                    cam_name, buf, det, conf, stats, rec, qg, gal, trk,
                )
                .await;
            });
        }
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

    // Initialize enrollment API state
    let enrollment_state = Arc::new(enrollment::service::EnrollmentState {
        db_path: config.recognition.gallery_db_path.clone(),
        gallery: Arc::clone(&gallery),
        detector: shared_detector.clone(),
        recognizer: recognizer.clone(),
        audit: Arc::clone(&audit_writer),
        quality_gates: QualityGates {
            min_face_size: config.enrollment.min_face_size,
            min_laplacian_var: config.enrollment.min_laplacian_var,
            max_yaw_degrees: config.enrollment.max_yaw_degrees,
        },
        config: config.enrollment.clone(),
        detection_confidence: config.detection.confidence_threshold,
    });

    if enrollment_state.detector.is_none() || enrollment_state.recognizer.is_none() {
        tracing::warn!("enrollment photo processing unavailable (missing detector or recognizer); CRUD endpoints still work");
    }

    let app = health::health_router(state)
        .merge(health::privacy_router(audit_writer.clone()))
        .merge(enrollment::routes::enrollment_router(enrollment_state));
    let addr = format!("{}:{}", config.service.host, config.service.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("rc-sentry-ai health endpoint listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
