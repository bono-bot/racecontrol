use std::sync::Arc;

use image::imageops::FilterType;
use image::DynamicImage;
use ndarray::Array4;
use ort::session::Session;
use ort::value::Tensor;
use tokio::sync::Mutex;

use super::types::DetectedFace;

/// SCRFD-10GF face detector using ONNX Runtime with CUDA execution provider.
///
/// The session is wrapped in `Arc<Mutex<Session>>` because `session.run()` requires
/// `&mut self` in ort 2.0. Create one `ScrfdDetector`, share via `clone_shared()`.
pub struct ScrfdDetector {
    session: Arc<Mutex<Session>>,
}

impl ScrfdDetector {
    /// Load SCRFD ONNX model with CUDA execution provider.
    ///
    /// Uses `error_on_failure()` so CUDA EP initialization failure is an error,
    /// not a silent fallback to CPU.
    pub fn new(model_path: &str) -> anyhow::Result<Self> {
        use ort::ep;

        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("ort session builder: {e}"))?
            .with_execution_providers([ep::CUDA::default().build().error_on_failure()])
            .map_err(|e| anyhow::anyhow!("ort CUDA EP: {e}"))?
            .commit_from_file(model_path)
            .map_err(|e| anyhow::anyhow!("ort commit model: {e}"))?;

        // Log all input/output node names and shapes for runtime verification
        for input in session.inputs() {
            tracing::info!(
                name = %input.name(),
                dtype = ?input.dtype(),
                "SCRFD input node"
            );
        }
        for output in session.outputs() {
            tracing::info!(
                name = %output.name(),
                dtype = ?output.dtype(),
                "SCRFD output node"
            );
        }

        Ok(Self {
            session: Arc::new(Mutex::new(session)),
        })
    }

    /// Clone the detector (Arc clone -- cheap, shares the underlying session).
    pub fn clone_shared(&self) -> Self {
        Self {
            session: Arc::clone(&self.session),
        }
    }

    /// Preprocess RGB pixels for SCRFD inference.
    ///
    /// - Resizes maintaining aspect ratio to fit within 640x640
    /// - Zero-pads to exactly 640x640 (bottom-right)
    /// - Normalizes: `(pixel - 127.5) / 128.0`
    /// - Returns NCHW tensor `[1, 3, 640, 640]` and scale factor for coordinate recovery
    pub fn preprocess(rgb: &[u8], width: u32, height: u32) -> Option<(Array4<f32>, f32)> {
        let Some(raw_img) = image::RgbImage::from_raw(width, height, rgb.to_vec()) else {
            tracing::warn!("SCRFD preprocess: RGB buffer size mismatch ({}x{}, got {} bytes)", width, height, rgb.len());
            return None;
        };
        let img = DynamicImage::from(raw_img);

        // Resize maintaining aspect ratio
        let scale = 640.0_f32 / width.max(height) as f32;
        let new_w = (width as f32 * scale) as u32;
        let new_h = (height as f32 * scale) as u32;
        let resized = img.resize_exact(new_w, new_h, FilterType::Lanczos3);

        // Create 640x640 canvas, paste resized image at (0,0), zero-pad remainder
        let mut canvas = image::RgbImage::new(640, 640);
        image::imageops::overlay(&mut canvas, &resized.to_rgb8(), 0, 0);

        // Convert to NCHW f32 with normalization: (pixel - 127.5) / 128.0
        let mut tensor = Array4::<f32>::zeros((1, 3, 640, 640));
        for y in 0..640u32 {
            for x in 0..640u32 {
                let pixel = canvas.get_pixel(x, y);
                tensor[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 - 127.5) / 128.0;
                tensor[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 - 127.5) / 128.0;
                tensor[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 - 127.5) / 128.0;
            }
        }

        Some((tensor, scale))
    }

    /// Run SCRFD inference and post-process detections.
    ///
    /// Uses `spawn_blocking` internally since ONNX inference is GPU-bound
    /// and would block the tokio runtime.
    ///
    /// # Arguments
    /// - `tensor`: Preprocessed NCHW tensor from `preprocess()`
    /// - `det_scale`: Scale factor from `preprocess()` for coordinate recovery
    /// - `conf_threshold`: Minimum confidence to keep a detection (default: 0.5)
    pub async fn detect(
        &self,
        tensor: Array4<f32>,
        det_scale: f32,
        conf_threshold: f32,
    ) -> anyhow::Result<Vec<DetectedFace>> {
        let session = Arc::clone(&self.session);

        let faces = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<DetectedFace>> {
            // Lock session for mutable access (ort 2.0 requires &mut self for run)
            let mut session = session.blocking_lock();

            // Create input tensor (takes ownership of the ndarray)
            let input_tensor = Tensor::from_array(tensor)
                .map_err(|e| anyhow::anyhow!("tensor creation: {e}"))?;

            let outputs = session
                .run(ort::inputs![input_tensor])
                .map_err(|e| anyhow::anyhow!("ort inference: {e}"))?;

            // SCRFD outputs 9 tensors across 3 FPN levels (strides 8, 16, 32):
            //   3 score tensors (last dim == 1)
            //   3 bbox tensors  (last dim == 4)
            //   3 kps tensors   (last dim == 10)
            //
            // We classify output tensors by their last dimension.
            let strides: [usize; 3] = [8, 16, 32];

            // Collect tensors as (shape_dims, flat_data) grouped by last dim
            let mut score_data: Vec<(Vec<i64>, Vec<f32>)> = Vec::new();
            let mut bbox_data: Vec<(Vec<i64>, Vec<f32>)> = Vec::new();
            let mut kps_data: Vec<(Vec<i64>, Vec<f32>)> = Vec::new();

            for (_name, value) in &outputs {
                let (shape, data) = value
                    .try_extract_tensor::<f32>()
                    .map_err(|e| anyhow::anyhow!("extract tensor: {e}"))?;
                let dims: Vec<i64> = shape.iter().copied().collect();
                let last_dim = *dims.last().unwrap_or(&0);
                match last_dim {
                    1 => score_data.push((dims, data.to_vec())),
                    4 => bbox_data.push((dims, data.to_vec())),
                    10 => kps_data.push((dims, data.to_vec())),
                    _ => {
                        tracing::warn!(last_dim, "unexpected SCRFD output tensor dimension");
                    }
                }
            }

            if score_data.len() != 3 || bbox_data.len() != 3 || kps_data.len() != 3 {
                anyhow::bail!(
                    "expected 3 score + 3 bbox + 3 kps tensors, got {} + {} + {}",
                    score_data.len(),
                    bbox_data.len(),
                    kps_data.len()
                );
            }

            let mut all_faces = Vec::new();

            for level in 0..3 {
                let stride = strides[level] as f32;
                let feat_h = 640 / strides[level];
                let feat_w = 640 / strides[level];

                let scores = &score_data[level].1;
                let bboxes = &bbox_data[level].1;
                let kps = &kps_data[level].1;

                // Generate anchor centers: 2 anchors per grid location
                let num_anchors_per_loc = 2;
                let mut anchor_idx = 0;

                for y in 0..feat_h {
                    for x in 0..feat_w {
                        for _ in 0..num_anchors_per_loc {
                            // Flat index for score (shape: [1, num_anchors, 1])
                            let score = scores[anchor_idx];
                            if score < conf_threshold {
                                anchor_idx += 1;
                                continue;
                            }

                            let anchor_cx = (x as f32 * stride) + (stride / 2.0);
                            let anchor_cy = (y as f32 * stride) + (stride / 2.0);

                            // Extract bbox distances (shape: [1, num_anchors, 4])
                            let b_off = anchor_idx * 4;
                            let dist = [
                                bboxes[b_off],
                                bboxes[b_off + 1],
                                bboxes[b_off + 2],
                                bboxes[b_off + 3],
                            ];
                            let bbox = distance2bbox((anchor_cx, anchor_cy), &dist, stride);

                            // Extract keypoint distances (shape: [1, num_anchors, 10])
                            let k_off = anchor_idx * 10;
                            let kp_dist: [f32; 10] = [
                                kps[k_off],
                                kps[k_off + 1],
                                kps[k_off + 2],
                                kps[k_off + 3],
                                kps[k_off + 4],
                                kps[k_off + 5],
                                kps[k_off + 6],
                                kps[k_off + 7],
                                kps[k_off + 8],
                                kps[k_off + 9],
                            ];
                            let landmarks =
                                distance2kps((anchor_cx, anchor_cy), &kp_dist, stride);

                            // Scale coordinates back to original image space
                            let face = DetectedFace {
                                bbox: [
                                    bbox[0] / det_scale,
                                    bbox[1] / det_scale,
                                    bbox[2] / det_scale,
                                    bbox[3] / det_scale,
                                ],
                                confidence: score,
                                landmarks: [
                                    [landmarks[0][0] / det_scale, landmarks[0][1] / det_scale],
                                    [landmarks[1][0] / det_scale, landmarks[1][1] / det_scale],
                                    [landmarks[2][0] / det_scale, landmarks[2][1] / det_scale],
                                    [landmarks[3][0] / det_scale, landmarks[3][1] / det_scale],
                                    [landmarks[4][0] / det_scale, landmarks[4][1] / det_scale],
                                ],
                            };
                            all_faces.push(face);

                            anchor_idx += 1;
                        }
                    }
                }
            }

            // Apply NMS to remove overlapping detections
            nms(&mut all_faces, 0.4);

            Ok(all_faces)
        })
        .await??;

        Ok(faces)
    }
}

/// Convert anchor center + distance offsets to bounding box (x1, y1, x2, y2).
///
/// dist: [left, top, right, bottom] distances from anchor center.
fn distance2bbox(anchor: (f32, f32), dist: &[f32], stride: f32) -> [f32; 4] {
    [
        anchor.0 - dist[0] * stride,
        anchor.1 - dist[1] * stride,
        anchor.0 + dist[2] * stride,
        anchor.1 + dist[3] * stride,
    ]
}

/// Convert anchor center + distance offsets to 5-point facial landmarks.
///
/// dist: 10 values (5 landmarks x 2 coordinates: dx, dy for each).
fn distance2kps(anchor: (f32, f32), dist: &[f32], stride: f32) -> [[f32; 2]; 5] {
    [
        [anchor.0 + dist[0] * stride, anchor.1 + dist[1] * stride],
        [anchor.0 + dist[2] * stride, anchor.1 + dist[3] * stride],
        [anchor.0 + dist[4] * stride, anchor.1 + dist[5] * stride],
        [anchor.0 + dist[6] * stride, anchor.1 + dist[7] * stride],
        [anchor.0 + dist[8] * stride, anchor.1 + dist[9] * stride],
    ]
}

/// Greedy Non-Maximum Suppression.
///
/// Sorts by confidence descending, then for each face suppresses all
/// lower-confidence faces with IoU > `iou_threshold`.
fn nms(faces: &mut Vec<DetectedFace>, iou_threshold: f32) {
    faces.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut keep = vec![true; faces.len()];

    for i in 0..faces.len() {
        if !keep[i] {
            continue;
        }
        for j in (i + 1)..faces.len() {
            if !keep[j] {
                continue;
            }
            if iou(&faces[i].bbox, &faces[j].bbox) > iou_threshold {
                keep[j] = false;
            }
        }
    }

    let mut idx = 0;
    faces.retain(|_| {
        let k = keep[idx];
        idx += 1;
        k
    });
}

/// Compute Intersection over Union between two bounding boxes.
///
/// Each bbox is `[x1, y1, x2, y2]`.
fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let x1 = a[0].max(b[0]);
    let y1 = a[1].max(b[1]);
    let x2 = a[2].min(b[2]);
    let y2 = a[3].min(b[3]);

    let inter_w = (x2 - x1).max(0.0);
    let inter_h = (y2 - y1).max(0.0);
    let inter_area = inter_w * inter_h;

    let area_a = (a[2] - a[0]) * (a[3] - a[1]);
    let area_b = (b[2] - b[0]) * (b[3] - b[1]);
    let union_area = area_a + area_b - inter_area;

    if union_area <= 0.0 {
        return 0.0;
    }

    inter_area / union_area
}
