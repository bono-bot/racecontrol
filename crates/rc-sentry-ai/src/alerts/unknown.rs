use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use tokio::sync::broadcast;

use super::types::{AlertEvent, UnknownFaceEvent};

/// Run the unknown person alert engine.
///
/// Receives `UnknownFaceEvent` from the detection pipeline, rate-limits per
/// camera (one alert per `rate_limit_secs`), saves a 112x112 JPEG face crop
/// to `face_crop_dir`, and emits `AlertEvent::UnknownPerson` on `alert_tx`.
pub async fn run(
    mut unknown_rx: broadcast::Receiver<UnknownFaceEvent>,
    alert_tx: broadcast::Sender<AlertEvent>,
    face_crop_dir: String,
    face_crop_quality: u8,
    rate_limit_secs: u64,
) {
    // Ensure face crop directory exists (idempotent)
    if let Err(e) = std::fs::create_dir_all(&face_crop_dir) {
        tracing::error!(dir = %face_crop_dir, error = %e, "failed to create face crop directory");
        return;
    }

    let rate_limit_duration = std::time::Duration::from_secs(rate_limit_secs);

    // Per-camera last-alert timestamp for rate limiting
    let mut last_alert: HashMap<String, Instant> = HashMap::new();

    // Track last cleanup time for periodic rate limit map pruning
    let mut last_cleanup = Instant::now();
    let cleanup_interval = std::time::Duration::from_secs(300);

    tracing::info!(
        crop_dir = %face_crop_dir,
        rate_limit_secs = rate_limit_secs,
        quality = face_crop_quality,
        "unknown person alert engine started"
    );

    loop {
        match unknown_rx.recv().await {
            Ok(event) => {
                // Periodic cleanup of stale rate limit entries (every 5 minutes)
                if last_cleanup.elapsed() >= cleanup_interval {
                    last_alert.retain(|_, ts| ts.elapsed() < rate_limit_duration);
                    last_cleanup = Instant::now();
                }

                // Rate limit check: skip if alert sent for this camera within window
                if let Some(last_ts) = last_alert.get(&event.camera) {
                    if last_ts.elapsed() < rate_limit_duration {
                        tracing::debug!(
                            camera = %event.camera,
                            remaining_secs = (rate_limit_duration - last_ts.elapsed()).as_secs(),
                            "unknown person alert rate-limited, skipping"
                        );
                        continue;
                    }
                }

                // Save face crop as JPEG using spawn_blocking for file I/O
                let crop_dir = face_crop_dir.clone();
                let camera = event.camera.clone();
                let quality = face_crop_quality;
                let rgb_data = event.face_crop_rgb;
                let width = event.crop_width;
                let height = event.crop_height;
                let ts = event.timestamp;

                // Format timestamp in IST for filename
                let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60)
                    .expect("valid IST offset");
                let ist_ts = ts.with_timezone(&ist_offset);
                let ts_str = ist_ts.format("%Y%m%d_%H%M%S").to_string();

                // Sanitize camera name for filename (replace non-alphanumeric with _)
                let safe_camera: String = camera
                    .chars()
                    .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
                    .collect();

                let filename = format!("unknown_{safe_camera}_{ts_str}.jpg");
                let crop_path = PathBuf::from(&crop_dir).join(&filename);
                let crop_path_str = crop_path.to_string_lossy().to_string();

                let save_result = tokio::task::spawn_blocking(move || -> Result<(), String> {
                    use image::codecs::jpeg::JpegEncoder;
                    use std::io::BufWriter;

                    let file = std::fs::File::create(&crop_path)
                        .map_err(|e| format!("create file: {e}"))?;
                    let buf_writer = BufWriter::new(file);
                    let mut encoder = JpegEncoder::new_with_quality(buf_writer, quality);
                    encoder
                        .encode(&rgb_data, width, height, image::ExtendedColorType::Rgb8)
                        .map_err(|e| format!("jpeg encode: {e}"))?;
                    Ok(())
                })
                .await;

                match save_result {
                    Ok(Ok(())) => {
                        tracing::info!(
                            camera = %camera,
                            path = %crop_path_str,
                            "unknown person face crop saved"
                        );
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(
                            camera = %camera,
                            error = e,
                            "failed to save unknown person face crop"
                        );
                        // Still emit the alert but without crop path
                        let alert = AlertEvent::UnknownPerson {
                            camera: camera.clone(),
                            crop_path: None,
                            timestamp: ts,
                        };
                        let _ = alert_tx.send(alert);
                        last_alert.insert(camera, Instant::now());
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!(
                            camera = %camera,
                            error = %e,
                            "face crop save task panicked"
                        );
                        continue;
                    }
                }

                // Emit alert with crop path
                let alert = AlertEvent::UnknownPerson {
                    camera: camera.clone(),
                    crop_path: Some(crop_path_str),
                    timestamp: ts,
                };
                let _ = alert_tx.send(alert);

                // Update rate limit map
                last_alert.insert(camera, Instant::now());
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::info!("unknown face broadcast channel closed, shutting down");
                break;
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "unknown face receiver lagged");
            }
        }
    }
}
