use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;

use super::scrfd::ScrfdDetector;
use crate::frame::FrameBuffer;

/// Shared detection stats visible to health endpoint.
pub struct DetectionStats {
    pub faces_detected: AtomicU64,
    pub frames_processed: AtomicU64,
    pub last_detection: RwLock<Option<Instant>>,
}

impl DetectionStats {
    pub fn new() -> Self {
        Self {
            faces_detected: AtomicU64::new(0),
            frames_processed: AtomicU64::new(0),
            last_detection: RwLock::new(None),
        }
    }
}

/// Run detection loop for a single camera.
///
/// Polls FrameBuffer, decodes H.264, runs SCRFD, logs detections.
/// Skips silently when no face detected (per user decision).
pub async fn run(
    camera_name: String,
    frame_buf: FrameBuffer,
    detector: Arc<ScrfdDetector>,
    conf_threshold: f32,
    stats: Arc<DetectionStats>,
) {
    let mut decoder = match super::decoder::FrameDecoder::new() {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(camera = %camera_name, error = %e, "failed to create H.264 decoder");
            return;
        }
    };

    let mut last_frame_count: u64 = 0;

    loop {
        // Poll for new frame
        let frame_data = match frame_buf.get(&camera_name).await {
            Some(fd) if fd.frame_count > last_frame_count => {
                last_frame_count = fd.frame_count;
                fd
            }
            _ => {
                // No new frame, short sleep to avoid busy-wait
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                continue;
            }
        };

        // Decode H.264 NAL to RGB
        let decoded = match decoder.decode(&frame_data.data) {
            Ok(Some(d)) => d,
            Ok(None) => continue, // Waiting for keyframe
            Err(e) => {
                tracing::debug!(camera = %camera_name, error = %e, "frame decode failed");
                continue;
            }
        };

        // Preprocess for SCRFD
        let (tensor, det_scale) =
            ScrfdDetector::preprocess(&decoded.rgb, decoded.width, decoded.height);

        // Run inference (ScrfdDetector::detect is async, uses spawn_blocking internally)
        let faces = match detector.detect(tensor, det_scale, conf_threshold).await {
            Ok(faces) => faces,
            Err(e) => {
                tracing::warn!(camera = %camera_name, error = %e, "detection inference failed");
                continue;
            }
        };

        stats.frames_processed.fetch_add(1, Ordering::Relaxed);

        // Skip silently when no faces (per user decision)
        if faces.is_empty() {
            continue;
        }

        // Log detections
        stats
            .faces_detected
            .fetch_add(faces.len() as u64, Ordering::Relaxed);
        *stats.last_detection.write().await = Some(Instant::now());

        tracing::info!(
            camera = %camera_name,
            face_count = faces.len(),
            "faces detected"
        );
        for (i, face) in faces.iter().enumerate() {
            tracing::debug!(
                camera = %camera_name,
                face_idx = i,
                confidence = face.confidence,
                bbox = ?face.bbox,
                "face detail"
            );
        }

        // Future phases will consume faces via broadcast channel.
        // For Phase 113, logging is sufficient.
    }
}
