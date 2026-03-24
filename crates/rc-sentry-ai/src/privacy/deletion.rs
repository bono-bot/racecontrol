use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};

use super::audit::{AuditEntry, AuditWriter};

/// Combined state for privacy routes: audit writer + DB path for actual deletion.
#[derive(Clone)]
pub struct PrivacyState {
    pub audit: Arc<AuditWriter>,
    pub db_path: String,
    pub face_crop_dir: String,
}

/// DELETE /api/v1/privacy/person/:person_id
/// DPDP Act 2023 Section 12 -- right to erasure.
/// Deletes: person record, face embeddings, attendance logs, face crops.
pub async fn delete_person_handler(
    Path(person_id): Path<String>,
    State(state): State<Arc<PrivacyState>>,
) -> Json<Value> {
    let person_id_i64 = match person_id.parse::<i64>() {
        Ok(id) => id,
        Err(_) => {
            return Json(json!({
                "status": "error",
                "message": "invalid person_id — must be an integer"
            }));
        }
    };

    // Log the deletion request in audit trail
    let entry = AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        action: "person_deleted".to_string(),
        person_id: Some(person_id.clone()),
        accessor: "api".to_string(),
        details: Some("DPDP right-to-deletion request".to_string()),
    };
    state.audit.log(entry);

    // Delete from SQLite (persons + embeddings via CASCADE + attendance)
    let db_path = state.db_path.clone();
    let pid = person_id_i64;
    let db_result = tokio::task::spawn_blocking(move || -> Result<(usize, usize, usize), String> {
        let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
        conn.execute("PRAGMA foreign_keys = ON", []).map_err(|e| e.to_string())?;

        // Count what we're deleting for the response
        let embedding_count: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM face_embeddings WHERE person_id = ?1",
                [pid],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let attendance_count: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM attendance_log WHERE person_id = ?1",
                [pid],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let shift_count: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM staff_shifts WHERE person_id = ?1",
                [pid],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Delete attendance records
        conn.execute("DELETE FROM attendance_log WHERE person_id = ?1", [pid])
            .map_err(|e| e.to_string())?;

        // Delete shift records
        conn.execute("DELETE FROM staff_shifts WHERE person_id = ?1", [pid])
            .map_err(|e| e.to_string())?;

        // Delete person (CASCADE deletes embeddings)
        conn.execute("DELETE FROM persons WHERE id = ?1", [pid])
            .map_err(|e| e.to_string())?;

        Ok((embedding_count, attendance_count, shift_count))
    })
    .await;

    // Delete face crop images for this person
    let crop_dir = std::path::Path::new(&state.face_crop_dir);
    if crop_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(crop_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains(&format!("person_{pid}_")) || name.contains(&format!("person-{pid}-")) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }

    let pid = person_id_i64;
    match db_result {
        Ok(Ok((embeddings, attendance, shifts))) => {
            tracing::info!(
                person_id = pid,
                embeddings_deleted = embeddings,
                attendance_deleted = attendance,
                shifts_deleted = shifts,
                "DPDP deletion completed"
            );
            Json(json!({
                "status": "deleted",
                "person_id": pid,
                "embeddings_deleted": embeddings,
                "attendance_records_deleted": attendance,
                "shift_records_deleted": shifts
            }))
        }
        Ok(Err(e)) => {
            tracing::error!(person_id = pid, error = %e, "DPDP deletion failed");
            Json(json!({
                "status": "error",
                "person_id": pid,
                "message": e
            }))
        }
        Err(e) => {
            tracing::error!(person_id = pid, error = %e, "DPDP deletion task panicked");
            Json(json!({
                "status": "error",
                "person_id": pid,
                "message": "internal error"
            }))
        }
    }
}
