use crate::recognition::types::RecognitionResult;

/// Intermediate event emitted by the detection pipeline when a face passes
/// quality gates but has no gallery match above the similarity threshold.
/// The unknown-person alert engine consumes these to save JPEG crops and
/// emit rate-limited `AlertEvent::UnknownPerson` alerts.
#[derive(Debug, Clone)]
pub struct UnknownFaceEvent {
    pub camera: String,
    /// 112x112 RGB aligned face pixels.
    pub face_crop_rgb: Vec<u8>,
    pub crop_width: u32,
    pub crop_height: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Real-time alert events sent to dashboard clients via WebSocket.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlertEvent {
    /// A known person was recognized by face recognition.
    Recognized {
        person_id: i64,
        person_name: String,
        confidence: f32,
        camera: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// An unknown person was detected (no gallery match).
    /// Variant defined now; populated by Plan 03.
    UnknownPerson {
        camera: String,
        crop_path: Option<String>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

impl From<RecognitionResult> for AlertEvent {
    fn from(r: RecognitionResult) -> Self {
        Self::Recognized {
            person_id: r.person_id,
            person_name: r.person_name,
            confidence: r.confidence,
            camera: r.camera,
            timestamp: r.timestamp,
        }
    }
}
