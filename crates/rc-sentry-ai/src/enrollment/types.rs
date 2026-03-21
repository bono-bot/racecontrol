use serde::{Deserialize, Serialize};

/// Request body for creating a new person.
#[derive(Debug, Deserialize)]
pub struct CreatePersonRequest {
    pub name: String,
    #[serde(default = "default_role")]
    pub role: String,
    #[serde(default)]
    pub phone: String,
}

fn default_role() -> String {
    "customer".to_string()
}

/// Request body for updating an existing person.
#[derive(Debug, Deserialize)]
pub struct UpdatePersonRequest {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub phone: String,
}

/// Response body for person details.
#[derive(Debug, Serialize)]
pub struct PersonResponse {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub phone: String,
    pub embedding_count: u64,
    pub enrollment_status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Response body after uploading a photo for enrollment.
#[derive(Debug, Serialize)]
pub struct PhotoUploadResponse {
    pub embedding_id: i64,
    pub embedding_count: u64,
    pub enrollment_status: String,
    pub duplicate_warning: Option<DuplicateWarning>,
}

/// Warning when an enrolled face closely matches an existing person.
#[derive(Debug, Serialize)]
pub struct DuplicateWarning {
    pub matched_person_id: i64,
    pub matched_person_name: String,
    pub similarity: f32,
}

/// Error response body.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Enrollment status based on embedding count (default threshold of 3).
pub fn enrollment_status(embedding_count: u64) -> &'static str {
    enrollment_status_with_threshold(embedding_count, 3)
}

/// Enrollment status based on embedding count with configurable threshold.
pub fn enrollment_status_with_threshold(embedding_count: u64, min_complete: u64) -> &'static str {
    if embedding_count >= min_complete {
        "complete"
    } else {
        "partial"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_create_person_request() {
        // With all fields
        let json = r#"{"name":"Alice","role":"staff","phone":"+91-123"}"#;
        let req: CreatePersonRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.name, "Alice");
        assert_eq!(req.role, "staff");
        assert_eq!(req.phone, "+91-123");

        // With defaults (role defaults to "customer", phone defaults to "")
        let json_minimal = r#"{"name":"Bob"}"#;
        let req2: CreatePersonRequest = serde_json::from_str(json_minimal).expect("deserialize");
        assert_eq!(req2.name, "Bob");
        assert_eq!(req2.role, "customer");
        assert_eq!(req2.phone, "");
    }

    #[test]
    fn test_serde_person_response() {
        let resp = PersonResponse {
            id: 1,
            name: "Alice".to_string(),
            role: "staff".to_string(),
            phone: "+91-123".to_string(),
            embedding_count: 3,
            enrollment_status: "complete".to_string(),
            created_at: "2026-01-01 00:00:00".to_string(),
            updated_at: "2026-01-01 00:00:00".to_string(),
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains("\"enrollment_status\":\"complete\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"name\":\"Alice\""));
        assert!(json.contains("\"phone\":\"+91-123\""));
    }

    #[test]
    fn test_enrollment_status_thresholds() {
        assert_eq!(enrollment_status(0), "partial");
        assert_eq!(enrollment_status(1), "partial");
        assert_eq!(enrollment_status(2), "partial");
        assert_eq!(enrollment_status(3), "complete");
        assert_eq!(enrollment_status(10), "complete");
    }

    #[test]
    fn test_error_response_skips_none_details() {
        let err = ErrorResponse {
            error: "not found".to_string(),
            details: None,
        };
        let json = serde_json::to_string(&err).expect("serialize");
        assert!(!json.contains("details"), "None details should be skipped");

        let err_with = ErrorResponse {
            error: "bad request".to_string(),
            details: Some("missing name".to_string()),
        };
        let json2 = serde_json::to_string(&err_with).expect("serialize");
        assert!(json2.contains("\"details\":\"missing name\""));
    }
}
