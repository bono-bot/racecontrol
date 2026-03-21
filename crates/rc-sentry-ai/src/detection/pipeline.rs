use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;

use super::scrfd::ScrfdDetector;
use crate::frame::FrameBuffer;
use crate::recognition::arcface::ArcfaceRecognizer;
use crate::recognition::gallery::Gallery;
use crate::recognition::quality::QualityGates;
use crate::recognition::tracker::FaceTracker;
use crate::recognition::{alignment, clahe};

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
    recognizer: Option<Arc<ArcfaceRecognizer>>,
    quality_gates: QualityGates,
    gallery: Arc<Gallery>,
    tracker: Arc<FaceTracker>,
    recognition_tx: Option<tokio::sync::broadcast::Sender<crate::recognition::types::RecognitionResult>>,
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

        // Run recognition pipeline if recognizer is available
        if let Some(ref recognizer) = recognizer {
            // Convert decoded RGB to grayscale for quality checks
            let decoded_gray: Vec<u8> = decoded
                .rgb
                .chunks_exact(3)
                .map(|px| {
                    (0.299 * px[0] as f64 + 0.587 * px[1] as f64 + 0.114 * px[2] as f64)
                        as u8
                })
                .collect();

            for face in &faces {
                // 1. Quality gate check
                if let Err(reason) = quality_gates.check(
                    face,
                    &decoded_gray,
                    decoded.width,
                    decoded.height,
                ) {
                    tracing::debug!(
                        camera = %camera_name,
                        reason = ?reason,
                        "face rejected by quality gates"
                    );
                    continue;
                }

                // 2. Align face to 112x112 using landmarks
                let aligned = alignment::align_face(
                    &decoded.rgb,
                    decoded.width,
                    decoded.height,
                    &face.landmarks,
                );

                // 3. Apply CLAHE for lighting normalization
                let clahe_face = clahe::apply_clahe(&aligned);

                // 4. Preprocess for ArcFace
                let tensor =
                    crate::recognition::arcface::preprocess(&clahe_face);

                // 5. Extract embedding
                let embedding = match recognizer.extract_embedding(tensor).await {
                    Ok(emb) => emb,
                    Err(e) => {
                        tracing::warn!(
                            camera = %camera_name,
                            error = %e,
                            "ArcFace embedding extraction failed"
                        );
                        continue;
                    }
                };

                // 6. Gallery match
                if let Some((person_id, person_name, confidence)) =
                    gallery.find_match(&embedding).await
                {
                    // 7. Tracker cooldown check
                    if tracker.should_report(person_id) {
                        tracing::info!(
                            camera = %camera_name,
                            person_name = %person_name,
                            person_id = person_id,
                            confidence = confidence,
                            "face recognized"
                        );
                        if let Some(ref tx) = recognition_tx {
                            let result = crate::recognition::types::RecognitionResult {
                                person_id,
                                person_name: person_name.clone(),
                                confidence,
                                camera: camera_name.clone(),
                                timestamp: chrono::Utc::now(),
                            };
                            let _ = tx.send(result);
                        }
                    }
                }
            }
        }
    }
}
