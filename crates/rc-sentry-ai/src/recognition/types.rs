/// Reason a face was rejected by quality gates.
#[derive(Debug, Clone, serde::Serialize)]
pub enum RejectReason {
    /// Face bounding box is smaller than minimum required dimensions.
    TooSmall { width: u32, height: u32 },
    /// Face crop is too blurry (low Laplacian variance).
    TooBlurry { laplacian_var: f64 },
    /// Face yaw angle exceeds maximum threshold.
    ExcessiveYaw { estimated_yaw: f64 },
}

/// Result of a successful face recognition match.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecognitionResult {
    pub person_id: i64,
    pub person_name: String,
    pub confidence: f32,
    pub camera: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Entry in the face embedding gallery.
#[derive(Debug, Clone)]
pub struct GalleryEntry {
    pub person_id: i64,
    pub person_name: String,
    pub embedding: [f32; 512],
}
