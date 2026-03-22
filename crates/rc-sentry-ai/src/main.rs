mod alerts;
mod attendance;
mod config;
mod detection;
mod enrollment;
mod frame;
mod health;
mod mjpeg;
mod nvr;
mod playback;
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

    if config.nvr.enabled {
        tracing::info!(host = %config.nvr.host, port = config.nvr.port, "NVR playback proxy configured");
    }

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

    // Create broadcast channel for recognition events (attendance, future consumers)
    let (recognition_tx, _) =
        tokio::sync::broadcast::channel::<recognition::types::RecognitionResult>(256);

    // Create broadcast channel for alert events (WebSocket fan-out)
    let (alert_tx, _) =
        tokio::sync::broadcast::channel::<alerts::types::AlertEvent>(256);

    // Create broadcast channel for unknown face events (pipeline -> unknown engine)
    let (unknown_tx, _) =
        tokio::sync::broadcast::channel::<alerts::types::UnknownFaceEvent>(64);

    // Spawn alert engine (if enabled)
    if config.alerts.enabled {
        let alert_rx = recognition_tx.subscribe();
        let atx = alert_tx.clone();
        tokio::spawn(async move {
            alerts::engine::run(alert_rx, atx).await;
        });

        // Spawn toast notification engine (Windows desktop notifications)
        let toast_rx = alert_tx.subscribe();
        tokio::spawn(alerts::toast::run(toast_rx));
        tracing::info!("toast notification engine started");

        // Spawn unknown person alert engine
        let unknown_rx = unknown_tx.subscribe();
        tokio::spawn(alerts::unknown::run(
            unknown_rx,
            alert_tx.clone(),
            config.alerts.face_crop_dir.clone(),
            config.alerts.face_crop_quality,
            config.alerts.unknown_rate_limit_secs,
        ));
        tracing::info!(
            crop_dir = %config.alerts.face_crop_dir,
            rate_limit_secs = config.alerts.unknown_rate_limit_secs,
            "unknown person alert engine started"
        );
    }

    // Spawn attendance engine (if enabled)
    if config.attendance.enabled {
        let attendance_rx = recognition_tx.subscribe();
        let att_db_path = config.recognition.gallery_db_path.clone();
        let att_config = config.attendance.clone();
        tokio::spawn(async move {
            attendance::engine::run(attendance_rx, att_db_path, att_config).await;
        });
        tracing::info!("attendance engine started");
    }

    // Spawn per-camera detection tasks (if detector available)
    if let Some(ref detector) = shared_detector {
        tracing::info!("spawning detection pipeline for {} cameras", config.cameras.len());
        for camera in config.cameras.iter() {
            let cam_name = camera.name.clone();
            tracing::info!(camera = %cam_name, "spawning detection task");
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
            let tx = recognition_tx.clone();
            let utx = unknown_tx.clone();
            tokio::spawn(async move {
                tracing::info!(camera = %cam_name, "detection pipeline started");
                detection::pipeline::run(
                    cam_name.clone(), buf, det, conf, stats, rec, qg, gal, trk, Some(tx), Some(utx),
                )
                .await;
                tracing::error!(camera = %cam_name, "detection pipeline exited unexpectedly");
            });
        }
    } else {
        tracing::warn!("no SCRFD detector available — detection pipeline NOT spawned");
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

    // Initialize attendance API state
    let attendance_state = Arc::new(attendance::routes::AttendanceState {
        db_path: config.recognition.gallery_db_path.clone(),
        present_timeout_secs: config.attendance.present_timeout_secs,
        min_shift_hours: config.attendance.min_shift_hours,
    });

    // Initialize alert WebSocket state
    let alert_ws_state = Arc::new(alerts::ws::AlertWsState {
        alert_tx: alert_tx.clone(),
    });

    // Initialize NVR snapshot cache + background fetcher (if NVR enabled)
    let snapshot_cache = Arc::new(mjpeg::SnapshotCache::new());
    let nvr_channels: u32 = 13;
    if config.nvr.enabled {
        let nvr_for_snapshots = Arc::new(nvr::NvrClient::new(&config.nvr));
        mjpeg::spawn_snapshot_fetcher(
            nvr_for_snapshots,
            Arc::clone(&snapshot_cache),
            nvr_channels,
        );
    }

    // Initialize camera layout state (persisted to camera-layout.json next to config)
    let layout_file_path = {
        let config_dir = std::path::Path::new(&config_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new(r"C:\RacingPoint"));
        config_dir.join("camera-layout.json")
    };
    let layout_state = Arc::new(mjpeg::LayoutState::load(layout_file_path));
    tracing::info!(
        path = %layout_state.file_path.display(),
        "camera layout state loaded"
    );

    let mjpeg_state = Arc::new(mjpeg::MjpegState {
        frame_buf: frame_buf.clone(),
        cameras: config.cameras.clone(),
        service_port: config.service.port,
        nvr_channels,
        snapshot_cache,
        layout_state,
    });

    // Initialize playback proxy state (if NVR enabled)
    let playback_state = if config.nvr.enabled {
        Some(Arc::new(playback::PlaybackState {
            nvr_client: nvr::NvrClient::new(&config.nvr),
            cameras: config.cameras.clone(),
            db_path: config.recognition.gallery_db_path.clone(),
        }))
    } else {
        None
    };

    let app = health::health_router(state)
        .merge(health::privacy_router(audit_writer.clone()))
        .merge(enrollment::routes::enrollment_router(enrollment_state))
        .merge(attendance::routes::attendance_router(attendance_state))
        .merge(alerts::ws::alerts_router(alert_ws_state))
        .merge(mjpeg::mjpeg_router(mjpeg_state));

    // Conditionally add playback routes
    let app = if let Some(ps) = playback_state {
        tracing::info!("NVR playback proxy enabled");
        app.merge(playback::playback_router(ps))
    } else {
        tracing::info!("NVR playback proxy disabled (nvr.enabled = false)");
        app
    };
    let addr = format!("{}:{}", config.service.host, config.service.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("rc-sentry-ai health endpoint listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
