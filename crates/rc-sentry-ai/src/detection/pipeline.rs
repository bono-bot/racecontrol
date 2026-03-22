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
    unknown_tx: Option<tokio::sync::broadcast::Sender<crate::alerts::types::UnknownFaceEvent>>,
) {
    tracing::info!(camera = %camera_name, "detection pipeline::run() entered");
    let mut decoder = match super::decoder::FrameDecoder::new() {
        Ok(d) => {
            tracing::info!(camera = %camera_name, "H.264 decoder created successfully");
            d
        }
        Err(e) => {
            tracing::error!(camera = %camera_name, error = %e, "failed to create H.264 decoder");
            return;
        }
    };

    let mut last_frame_count: u64 = 0;
    let mut log_first_frame = true;
    let mut poll_count: u64 = 0;
    let mut consecutive_decode_errors: u32 = 0;
    const DECODER_RESET_THRESHOLD: u32 = 50;

    loop {
        // Poll for new frame
        let frame_data = match frame_buf.get(&camera_name).await {
            Some(fd) if fd.frame_count > last_frame_count => {
                if log_first_frame {
                    tracing::info!(camera = %camera_name, frame_count = fd.frame_count, data_len = fd.data.len(), "got first frame from buffer");
                }
                last_frame_count = fd.frame_count;
                fd
            }
            other => {
                poll_count += 1;
                if poll_count % 200 == 1 {
                    let (is_none, fc) = match &other {
                        None => (true, 0),
                        Some(fd) => (false, fd.frame_count),
                    };
                    tracing::debug!(camera = %camera_name, is_none, frame_count = fc, last = last_frame_count, polls = poll_count, "no new frame");
                }
                // No new frame, short sleep to avoid busy-wait
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                continue;
            }
        };

        // Decode H.264 NAL to RGB
        let decoded = match decoder.decode(&frame_data.data) {
            Ok(Some(d)) => {
                consecutive_decode_errors = 0;
                if log_first_frame {
                    tracing::info!(camera = %camera_name, w = d.width, h = d.height, rgb_len = d.rgb.len(), "first successful decode");
                }
                d
            }
            Ok(None) => {
                if log_first_frame && frame_data.frame_count % 50 == 0 {
                    tracing::debug!(camera = %camera_name, frame = frame_data.frame_count, data_len = frame_data.data.len(), "decode returned None (waiting for keyframe)");
                }
                continue;
            }
            Err(e) => {
                consecutive_decode_errors += 1;
                if consecutive_decode_errors <= 3 || consecutive_decode_errors % 100 == 0 {
                    tracing::warn!(
                        camera = %camera_name,
                        error = %e,
                        data_len = frame_data.data.len(),
                        frame = frame_data.frame_count,
                        consecutive_errors = consecutive_decode_errors,
                        "frame decode failed"
                    );
                }
                if consecutive_decode_errors >= DECODER_RESET_THRESHOLD {
                    tracing::warn!(
                        camera = %camera_name,
                        errors = consecutive_decode_errors,
                        "resetting decoder after sustained errors"
                    );
                    decoder = match super::decoder::FrameDecoder::new() {
                        Ok(d) => d,
                        Err(e) => {
                            tracing::error!(camera = %camera_name, error = %e, "failed to recreate decoder");
                            return;
                        }
                    };
                    consecutive_decode_errors = 0;
                }
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
        if log_first_frame {
            tracing::info!(camera = %camera_name, faces = faces.len(), "first frame processed by detection pipeline");
            log_first_frame = false;
        }

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

                // Keep a copy before CLAHE for unknown-person face crop
                let aligned_raw = aligned.clone();

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
                } else {
                    // No match above threshold — unknown person
                    if let Some(ref utx) = unknown_tx {
                        let event = crate::alerts::types::UnknownFaceEvent {
                            camera: camera_name.clone(),
                            face_crop_rgb: aligned_raw.into_raw(),
                            crop_width: 112,
                            crop_height: 112,
                            timestamp: chrono::Utc::now(),
                        };
                        let _ = utx.send(event);
                    }
                }
            }
        }
    }
}
