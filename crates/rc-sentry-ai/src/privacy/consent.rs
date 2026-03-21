use axum::Json;
use serde_json::{json, Value};

/// DPDP Act 2023 compliant signage text.
/// Must be displayed at premises entrance and on digital dashboard.
pub const SIGNAGE_TEXT: &str = "\
NOTICE: This premises uses CCTV with face recognition for security purposes.\n\
Your facial data is processed under DPDP Act 2023, Section 7(g) (legitimate use for safety/security).\n\
Data is retained for 90 days and automatically deleted.\n\
To request deletion of your data, contact: usingh@racingpoint.in\n\
Data Fiduciary: Racing Point eSports, Hyderabad";

/// GET /api/v1/privacy/consent -- returns consent notice text
pub async fn consent_notice_handler() -> Json<Value> {
    Json(json!({
        "notice": SIGNAGE_TEXT,
        "act": "Digital Personal Data Protection Act, 2023",
        "purpose": "security",
        "retention_days": 90,
        "contact": "usingh@racingpoint.in",
        "fiduciary": "Racing Point eSports, Hyderabad",
    }))
}
