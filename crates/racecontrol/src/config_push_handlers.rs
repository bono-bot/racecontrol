/// REST handlers for field-level config push, queue viewing, and audit log.

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::auth::middleware::StaffClaims;
use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage};
use rc_common::types::ConfigPushPayload;

use super::config_push_types::{
    AuditLogEntry, ConfigQueueEntry, PushConfigRequest, validate_config_push,
};

// ─── REST Handlers ────────────────────────────────────────────────────────────

/// POST /api/v1/config/push
/// Validate, queue, and deliver a config push to all or selected pods.
pub async fn push_config(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<StaffClaims>,
    Json(body): Json<PushConfigRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // 1. Validate fields
    if let Err(field_errors) = validate_config_push(&body.fields) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "errors": field_errors })),
        ));
    }

    // 2. Determine target pods
    let target_pods: Vec<String> = if body.target_pods.is_empty() {
        state.pods.read().await.keys().cloned().collect()
    } else {
        body.target_pods.clone()
    };

    // 3. Single seq_num for this push batch
    let seq_num = state.config_push_seq.fetch_add(1, Ordering::SeqCst);

    let payload_json = serde_json::to_string(&body.fields).unwrap_or_else(|_| "{}".to_string());

    let mut queued = 0usize;
    let mut delivered = 0usize;

    // 4. Queue and deliver per pod
    for pod_id in &target_pods {
        // Insert into config_push_queue
        let insert_result = sqlx::query(
            "INSERT INTO config_push_queue (pod_id, payload, seq_num, status) VALUES (?, ?, ?, 'pending')",
        )
        .bind(pod_id)
        .bind(&payload_json)
        .bind(seq_num as i64)
        .execute(&state.db)
        .await;

        if let Err(e) = insert_result {
            tracing::error!("Failed to insert config_push_queue entry for pod {}: {}", pod_id, e);
            continue;
        }
        queued += 1;

        // Deliver if pod is connected
        let sender = state.agent_senders.read().await.get(pod_id).cloned();
        if let Some(tx) = sender {
            let push_payload = ConfigPushPayload {
                fields: body.fields.clone(),
                schema_version: body.schema_version,
                sequence: seq_num,
            };
            match tx.send(CoreMessage::wrap(CoreToAgentMessage::ConfigPush(push_payload))).await {
                Ok(_) => {
                    delivered += 1;
                    // Update status to delivered
                    let _ = sqlx::query(
                        "UPDATE config_push_queue SET status = 'delivered' WHERE pod_id = ? AND seq_num = ?",
                    )
                    .bind(pod_id)
                    .bind(seq_num as i64)
                    .execute(&state.db)
                    .await;
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to send ConfigPush to pod {} (seq={}): {}",
                        pod_id, seq_num, e
                    );
                }
            }
        }
        // If pod not in agent_senders, leave status='pending' (offline pod: CP-02)
    }

    // 5. Write audit log entry
    let field_keys: Vec<&str> = body.fields.keys().map(|s| s.as_str()).collect();
    let entity_name = field_keys.join(",");
    let _ = sqlx::query(
        "INSERT INTO config_audit_log \
         (action, entity_type, entity_name, old_value, new_value, pushed_by, pods_acked, seq_num) \
         VALUES ('config_push', 'config', ?, NULL, ?, ?, '[]', ?)",
    )
    .bind(&entity_name)
    .bind(&payload_json)
    .bind(&claims.sub)
    .bind(seq_num as i64)
    .execute(&state.db)
    .await;

    Ok(Json(json!({
        "queued": queued,
        "delivered": delivered,
        "seq_nums": [seq_num]
    })))
}

/// GET /api/v1/config/push/queue
pub async fn get_queue(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ConfigQueueEntry>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, ConfigQueueEntry>(
        "SELECT id, pod_id, payload, seq_num, status, created_at, acked_at \
         FROM config_push_queue ORDER BY id DESC LIMIT 100",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

/// GET /api/v1/config/audit
pub async fn get_audit_log(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<AuditLogEntry>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, AuditLogEntry>(
        "SELECT id, action, entity_type, entity_name, old_value, new_value, \
         pushed_by, pods_acked, seq_num, created_at \
         FROM config_audit_log ORDER BY id DESC LIMIT 100",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}
