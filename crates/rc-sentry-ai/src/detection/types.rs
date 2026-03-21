/// Detected face from SCRFD model.
/// Coordinates are in original image space (not 640x640 tensor space).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DetectedFace {
    /// Bounding box: (x1, y1, x2, y2) in original image coordinates
    pub bbox: [f32; 4],
    /// Detection confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// 5-point facial landmarks: left_eye, right_eye, nose, left_mouth, right_mouth
    /// Each point is (x, y) in original image coordinates
    pub landmarks: [[f32; 2]; 5],
}
