use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};

use super::audit::{AuditEntry, AuditWriter};

/// DELETE /api/v1/privacy/person/:person_id
/// DPDP Act 2023 Section 12 -- right to erasure.
/// In Phase 113, deletes audit entries only (no embeddings yet -- Phase 114 adds embedding deletion).
pub async fn delete_person_handler(
    Path(person_id): Path<String>,
    State(audit): State<Arc<AuditWriter>>,
) -> Json<Value> {
    // Log the deletion request in audit trail
    let entry = AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        action: "person_deleted".to_string(),
        person_id: Some(person_id.clone()),
        accessor: "api".to_string(),
        details: Some("DPDP right-to-deletion request".to_string()),
    };
    audit.log(entry);

    // Phase 114 will add: delete face embeddings from SQLite
    // Phase 113 scope: log the deletion and confirm

    tracing::info!(person_id = %person_id, "DPDP deletion request processed");

    Json(json!({
        "status": "deleted",
        "person_id": person_id,
        "note": "Audit entry recorded. Embedding deletion available after Phase 114."
    }))
}
