use std::fmt;
use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::config::EnrollmentConfig;
use crate::detection::scrfd::ScrfdDetector;
use crate::privacy::audit::{AuditEntry, AuditWriter};
use crate::recognition::arcface::{self, ArcfaceRecognizer};
use crate::recognition::alignment::align_face;
use crate::recognition::clahe::apply_clahe;
use crate::recognition::db;
use crate::recognition::gallery::Gallery;
use crate::recognition::quality::QualityGates;
use crate::recognition::types::{GalleryEntry, RejectReason};

use super::types::{DuplicateWarning, PhotoUploadResponse};

/// Shared state for all enrollment handlers.
pub struct EnrollmentState {
    pub db_path: String,
    pub gallery: Arc<Gallery>,
    pub detector: Option<Arc<ScrfdDetector>>,
    pub recognizer: Option<Arc<ArcfaceRecognizer>>,
    pub audit: Arc<AuditWriter>,
    pub quality_gates: QualityGates,
    pub config: EnrollmentConfig,
    pub detection_confidence: f32,
}

/// Enrollment operation errors with appropriate HTTP status codes.
#[derive(Debug)]
pub enum EnrollmentError {
    BadImage(String),
    NoFaceDetected,
    MultipleFaces(usize),
    QualityRejected(RejectReason),
    PersonNotFound,
    ServiceUnavailable(String),
    Internal(String),
}

impl fmt::Display for EnrollmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadImage(msg) => write!(f, "bad image: {msg}"),
            Self::NoFaceDetected => write!(f, "no face detected in image"),
            Self::MultipleFaces(n) => write!(f, "expected 1 face, detected {n}"),
            Self::QualityRejected(reason) => write!(f, "quality rejected: {reason:?}"),
            Self::PersonNotFound => write!(f, "person not found"),
            Self::ServiceUnavailable(msg) => write!(f, "service unavailable: {msg}"),
            Self::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl IntoResponse for EnrollmentError {
    fn into_response(self) -> Response {
        let (status, error_msg, details) = match &self {
            Self::BadImage(msg) => (StatusCode::BAD_REQUEST, "bad_image", Some(msg.clone())),
            Self::NoFaceDetected => (StatusCode::UNPROCESSABLE_ENTITY, "no_face_detected", None),
            Self::MultipleFaces(n) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "multiple_faces",
                Some(format!("detected {n} faces, expected exactly 1")),
            ),
            Self::QualityRejected(reason) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "quality_rejected",
                Some(format!("{reason:?}")),
            ),
            Self::PersonNotFound => (StatusCode::NOT_FOUND, "person_not_found", None),
            Self::ServiceUnavailable(msg) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                Some(msg.clone()),
            ),
            Self::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                Some(msg.clone()),
            ),
        };

        let body = json!({
            "error": error_msg,
            "details": details,
        });

        (status, axum::Json(body)).into_response()
    }
}

/// Process a photo upload: decode, detect face, quality check, align, CLAHE, embed, duplicate check, persist.
pub async fn process_photo(
    state: &EnrollmentState,
    person_id: i64,
    image_bytes: &[u8],
) -> Result<PhotoUploadResponse, EnrollmentError> {
    // Check ML models are available
    let detector = state
        .detector
        .as_ref()
        .ok_or_else(|| EnrollmentError::ServiceUnavailable("SCRFD detector not loaded".into()))?;
    let recognizer = state
        .recognizer
        .as_ref()
        .ok_or_else(|| EnrollmentError::ServiceUnavailable("ArcFace recognizer not loaded".into()))?;

    // Verify person exists
    let db_path = state.db_path.clone();
    let pid = person_id;
    let person_name = tokio::task::spawn_blocking(move || -> Result<String, EnrollmentError> {
        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| EnrollmentError::Internal(format!("db open: {e}")))?;
        let person = db::get_person(&conn, pid)
            .map_err(|e| EnrollmentError::Internal(format!("db get_person: {e}")))?
            .ok_or(EnrollmentError::PersonNotFound)?;
        Ok(person.name)
    })
    .await
    .map_err(|e| EnrollmentError::Internal(format!("spawn_blocking: {e}")))??;

    // Decode image
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| EnrollmentError::BadImage(format!("image decode failed: {e}")))?
        .to_rgb8();

    let (width, height) = (img.width(), img.height());
    let rgb_bytes = img.into_raw();

    // Preprocess for SCRFD
    let (tensor, det_scale) = ScrfdDetector::preprocess(&rgb_bytes, width, height);

    // Detect faces
    let conf = state.detection_confidence;
    let det = detector.clone_shared();
    let faces = det
        .detect(tensor, det_scale, conf)
        .await
        .map_err(|e| EnrollmentError::Internal(format!("detection failed: {e}")))?;

    // Require exactly 1 face
    if faces.is_empty() {
        return Err(EnrollmentError::NoFaceDetected);
    }
    if faces.len() > 1 {
        return Err(EnrollmentError::MultipleFaces(faces.len()));
    }

    let face = &faces[0];

    // Quality gates with enrollment-specific stricter thresholds
    // Convert RGB to grayscale for quality check
    let gray: Vec<u8> = rgb_bytes
        .chunks_exact(3)
        .map(|px| {
            let r = px[0] as f32;
            let g = px[1] as f32;
            let b = px[2] as f32;
            (0.299 * r + 0.587 * g + 0.114 * b) as u8
        })
        .collect();

    state
        .quality_gates
        .check(face, &gray, width, height)
        .map_err(EnrollmentError::QualityRejected)?;

    // Align face
    let aligned = align_face(&rgb_bytes, width, height, &face.landmarks);

    // Apply CLAHE
    let enhanced = apply_clahe(&aligned);

    // ArcFace preprocessing
    let tensor = arcface::preprocess(&enhanced);

    // Extract embedding
    let rec = recognizer.clone_shared();
    let embedding = rec
        .extract_embedding(tensor)
        .await
        .map_err(|e| EnrollmentError::Internal(format!("embedding extraction: {e}")))?;

    // Duplicate check
    let duplicate_warning = {
        let match_result = state.gallery.find_match(&embedding).await;
        match match_result {
            Some((matched_id, matched_name, similarity))
                if similarity > state.config.duplicate_threshold && matched_id != person_id =>
            {
                Some(DuplicateWarning {
                    matched_person_id: matched_id,
                    matched_person_name: matched_name,
                    similarity,
                })
            }
            _ => None,
        }
    };

    // Persist embedding to DB
    let db_path = state.db_path.clone();
    let retention_days = state.config.retention_days;
    let min_complete = state.config.min_embeddings_complete;
    let (embedding_id, emb_count) = tokio::task::spawn_blocking(move || -> Result<(i64, u64), EnrollmentError> {
        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| EnrollmentError::Internal(format!("db open: {e}")))?;
        let eid = db::insert_embedding(&conn, person_id, &embedding, retention_days)
            .map_err(|e| EnrollmentError::Internal(format!("db insert_embedding: {e}")))?;
        let count = db::embedding_count(&conn, person_id)
            .map_err(|e| EnrollmentError::Internal(format!("db embedding_count: {e}")))?;
        Ok((eid, count))
    })
    .await
    .map_err(|e| EnrollmentError::Internal(format!("spawn_blocking: {e}")))??;

    // Sync gallery
    state
        .gallery
        .add_entry(GalleryEntry {
            person_id,
            person_name: person_name.clone(),
            embedding,
        })
        .await;

    // Audit log
    let status = super::types::enrollment_status_with_threshold(emb_count, min_complete);
    state.audit.log(AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        action: "photo_enrolled".to_string(),
        person_id: Some(person_id.to_string()),
        accessor: "api:enrollment".to_string(),
        details: Some(format!(
            "embedding_id={embedding_id}, count={emb_count}, status={status}"
        )),
    });

    Ok(PhotoUploadResponse {
        embedding_id,
        embedding_count: emb_count,
        enrollment_status: status.to_string(),
        duplicate_warning,
    })
}
