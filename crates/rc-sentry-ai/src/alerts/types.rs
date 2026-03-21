use crate::recognition::types::RecognitionResult;

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
